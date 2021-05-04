use std::{collections::HashMap, net::SocketAddr, pin::Pin, sync::Arc, time::Duration};

use futures::{channel::mpsc, future, pin_mut, Future, FutureExt, SinkExt, Stream, StreamExt};
use log::{debug, info, warn};
use rocket::http;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{oneshot, Mutex, MutexGuard},
};
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};

use crate::{error::Result, events::Event};

pub struct WebSocketServer {
    pub port: u16,
    dynamic_streams_waiting:
        Arc<Mutex<HashMap<String, oneshot::Sender<WebSocketStream<TcpStream>>>>>,
}

impl WebSocketServer {
    pub async fn new(bind_addr: SocketAddr) -> Result<Arc<WebSocketServer>> {
        let tcp_listener = TcpListener::bind(bind_addr).await?;

        let server = Arc::new(WebSocketServer {
            port: bind_addr.port(),
            dynamic_streams_waiting: Arc::new(Mutex::new(HashMap::new())),
        });

        let server_clone = Arc::clone(&server);
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {
                        info!("WebSocketServer received SIGINT");
                        break;
                    },
                    accept_res = tcp_listener.accept() => {
                        if let Ok((tcp, addr)) = accept_res {
                            debug!("WebSocketServer received connection request from {}", addr);
                            server_clone.route(tcp).await;
                        }
                    },
                }
            }
        });

        Ok(server)
    }

    pub async fn stream_at<S>(&self, path: String, stream: S, unconnected_timeout: Duration)
    where
        S: Stream<Item = Event> + Unpin + Send,
    {
        let (tx, rx) = oneshot::channel();

        {
            let mut mg = self.dynamic_streams_waiting.lock().await;
            mg.insert(path.clone(), tx);
        }

        match tokio::time::timeout(unconnected_timeout, rx).await {
            Ok(res) => {
                if let Ok(ws) = res {
                    debug!("WebSocket peer connected");
                    let (mut ws_tx, mut ws_rx) = ws.split();

                    // 1 hour for inactivity timeout, even if client is connected
                    let (mut activity_tx, mut activity_rx) = mpsc::unbounded();
                    let path = path.clone();
                    let inactivity_task = tokio::spawn(async move {
                        let inactivity_timeout = Duration::from_secs(60 * 60);
                        while let Ok(_) =
                            tokio::time::timeout(inactivity_timeout, activity_rx.next()).await
                        {
                        }
                        debug!("WebSocket stream at {} timing out from inactivity", path);
                    });

                    // Abstract ws_tx with a channel to avoid locking
                    let (mut outgoing_tx, mut outgoing_rx) = mpsc::unbounded();
                    tokio::spawn(async move {
                        while let Some(msg) = outgoing_rx.next().await {
                            let _ = activity_tx.send(()).await;
                            debug!("Sending into ws_tx: {}", msg);
                            let _ = ws_tx.send(msg).await;
                        }

                        debug!("Closing websocket connection");
                        let _ = ws_tx.send(Message::Close(None)).await;
                        let _ = ws_tx.close().await;
                    });

                    // Forward messages from stream to outgoing channel
                    let outgoing_tx_clone = outgoing_tx.clone();
                    pin_mut!(stream);
                    let forward_fut = stream.for_each(|e| {
                        let msg = Message::Text(e.content);
                        let _ = outgoing_tx_clone.unbounded_send(msg);
                        future::ready(())
                    });

                    // Handle incoming messages
                    let handle_incoming_task = tokio::spawn(async move {
                        while let Some(Ok(msg)) = ws_rx.next().await {
                            match msg {
                                Message::Text(_) | Message::Binary(_) | Message::Pong(_) => {
                                    // ignore
                                }
                                Message::Ping(_) => {
                                    // tungstenite library handles pings already
                                }
                                Message::Close(_) => {
                                    info!("Got close message");
                                    break;
                                }
                            }
                        }
                    });

                    // Wait until the forwarded stream is done, client closes connection, or timeout from inactivity.
                    // Eiher way, we are done, close the outgoing channel to close the connection.
                    let futures: Vec<Pin<Box<dyn Future<Output = ()> + Send>>> = vec![
                        Box::pin(forward_fut.then(|_| future::ready(()))),
                        Box::pin(handle_incoming_task.then(|_| future::ready(()))),
                        Box::pin(inactivity_task.then(|_| future::ready(()))),
                    ];
                    future::select_all(futures).await;
                    let _ = outgoing_tx.close().await;
                }
            }
            Err(_) => {
                // no-one connected, timed out
                // remove the entry
                let mut mg = self.dynamic_streams_waiting.lock().await;
                mg.remove(&path);
                info!(
                    "websocket stream '{}' timed out waiting for connection",
                    path
                );
            }
        }

        info!("Done with stream at: '{}'", path);
    }

    async fn route(&self, tcp: TcpStream) {
        let mg = self.dynamic_streams_waiting.lock().await;
        let (tx, rx) = oneshot::channel();
        let callback = DynamicStreamAcceptCallback {
            mutex_guard: mg,
            tx_ws_tx: tx,
        };
        match tokio_tungstenite::accept_hdr_async(tcp, callback).await {
            Ok(ws) => {
                let ws_tx = rx.await.unwrap(); // Safe to unwrap here

                // Send the websocket to the closure defined in stream_at()
                let _ = ws_tx.send(ws);
            }
            Err(e) => {
                warn!("WS connection request not accepted: {:?}", e);
            }
        }
    }
}

struct DynamicStreamAcceptCallback<'a> {
    mutex_guard: MutexGuard<'a, HashMap<String, oneshot::Sender<WebSocketStream<TcpStream>>>>,
    tx_ws_tx: oneshot::Sender<oneshot::Sender<WebSocketStream<TcpStream>>>,
}

impl<'a> tokio_tungstenite::tungstenite::handshake::server::Callback
    for DynamicStreamAcceptCallback<'a>
{
    fn on_request(
        mut self,
        request: &tokio_tungstenite::tungstenite::handshake::server::Request,
        response: tokio_tungstenite::tungstenite::handshake::server::Response,
    ) -> std::result::Result<
        tokio_tungstenite::tungstenite::handshake::server::Response,
        tokio_tungstenite::tungstenite::handshake::server::ErrorResponse,
    > {
        let path = request.uri().path();
        debug!("checking route for path: {}", path);
        if let Some(ws_tx) = self.mutex_guard.remove(path) {
            // Pass the websocket sender out of this callback, back to the route() function
            let _ = self.tx_ws_tx.send(ws_tx);
            Ok(response)
        } else {
            let mut response = http::hyper::Response::new(Some("no such route".to_owned()));
            *response.status_mut() = http::hyper::StatusCode::BAD_REQUEST;
            Err(response)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};

    use futures::stream;

    use super::*;

    #[tokio::test]
    async fn can_timeout_on_stream_at() {
        let bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8378);
        let s = WebSocketServer::new(bind_addr).await.unwrap();

        // stream_at() should time out with the internal timeout of 200ms, completing the future
        // before the external timeout of 500ms
        let external_timed_out = tokio::time::timeout(Duration::from_millis(500), async move {
            s.stream_at(
                "test".to_owned(),
                stream::repeat(Event {
                    tags: HashMap::new(),
                    content: "asdf".to_owned(),
                }),
                Duration::from_millis(200),
            )
            .await;
        })
        .await
        .is_err();

        assert!(!external_timed_out);
    }
}
