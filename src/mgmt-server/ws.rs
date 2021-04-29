use std::{collections::{HashMap, HashSet}, time::Duration};

use futures::Stream;
use tokio::{net::TcpStream, sync::{RwLock, oneshot}};
use tokio_tungstenite::WebSocketStream;

use crate::{error::Result, events::Event};

struct WebSocketRouter {
    dynamic_streams_waiting: RwLock<HashMap<String, oneshot::Sender<WebSocketStream<TcpStream>>>>,
}

impl WebSocketRouter {
    pub async fn new() -> Result<WebSocketRouter> {
        todo!()
    }

    pub async fn stream_at<S>(&self, path: String, stream: S, unconnected_timeout: Duration) -> Result<()>
    where
        S: Stream<Item = Event>,
    {
        let (tx, rx) = oneshot::channel();

        {
            let mut write_guard = self.dynamic_streams_waiting.write().await;
            write_guard.insert(path, tx);
        }

        if let Err(_) = tokio::time::timeout(unconnected_timeout, async move {
            let ws = rx.await;
            todo!()
        }).await {
            // no-one connected, timed out
            todo!()
        }

        todo!()
    }
}
