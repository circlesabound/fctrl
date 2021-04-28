use std::time::Duration;

use futures::Stream;

use crate::{error::Result, events::Event};

struct WebSocketRouter {
    //
}

impl WebSocketRouter {
    pub async fn new() -> Result<WebSocketRouter> {
        todo!()
    }

    pub async fn stream_at<S>(path: String, stream: S, unconnected_timeout: Duration) -> Result<()>
    where
        S: Stream<Item = Event>,
    {
        todo!()
    }
}
