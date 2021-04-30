use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};

use futures::Stream;
use log::{info, warn};
use rocket::http;
use serde::Serialize;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{oneshot, RwLock, RwLockWriteGuard},
};
use tokio_tungstenite::WebSocketStream;

use crate::error::Result;

pub struct WebSocketServer {
    dynamic_streams_waiting:
        Arc<RwLock<HashMap<String, oneshot::Sender<WebSocketStream<TcpStream>>>>>,
}

impl WebSocketServer {
    pub async fn new(bind_addr: SocketAddr) -> Result<Arc<WebSocketServer>> {
        let tcp_listener = TcpListener::bind(bind_addr).await?;

        let server = Arc::new(WebSocketServer {
            dynamic_streams_waiting: Arc::new(RwLock::new(HashMap::new())),
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
                            server_clone.route(tcp).await;
                        }
                        todo!()
                    },
                }
            }
        });

        Ok(server)
    }

    pub async fn stream_at<S, I>(&self, path: String, stream: S, unconnected_timeout: Duration)
    where
        S: Stream<Item = I>, // TODO a proper item type
        I: Serialize,
    {
        let (tx, rx) = oneshot::channel();

        {
            let mut write_guard = self.dynamic_streams_waiting.write().await;
            write_guard.insert(path.clone(), tx);
        }

        let timed_out = tokio::time::timeout(unconnected_timeout, async move {
            if let Ok(ws) = rx.await {
                info!("WebSocket peer connected");
                // TODO stream, handle disconnenct, ping, etc
                todo!()
            }
        })
        .await
        .is_err();

        if timed_out {
            // no-one connected, timed out
            // remove the entry
            let mut write_guard = self.dynamic_streams_waiting.write().await;
            write_guard.remove(&path);
            info!(
                "dynamic websocket stream '{}' timed out waiting for connection",
                path
            );
        }
    }

    async fn route(&self, tcp: TcpStream) {
        let write_guard = self.dynamic_streams_waiting.write().await;
        let (tx, rx) = oneshot::channel();
        let callback = DynamicStreamAcceptCallback {
            write_guard,
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
    write_guard: RwLockWriteGuard<'a, HashMap<String, oneshot::Sender<WebSocketStream<TcpStream>>>>,
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
        if let Some(ws_tx) = self.write_guard.remove(path) {
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
                stream::repeat(1),
                Duration::from_millis(200),
            )
            .await;
        })
        .await
        .is_err();

        assert!(!external_timed_out);
    }
}
