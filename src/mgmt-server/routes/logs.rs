use std::{sync::Arc, time::Duration};

use fctrl::schema::{mgmt_server_rest::LogsPaginationObject, OperationId};
use rocket::{get, State};
use rocket_contrib::json::Json;
use uuid::Uuid;

use crate::{
    error::Result,
    events::{broker::EventBroker, TopicName, STDOUT_TOPIC_NAME},
    guards::HostHeader,
    routes::WsStreamingResponder,
    ws::WebSocketServer,
};

#[get("/logs/<category>")]
pub async fn get(category: String) -> Result<Json<LogsPaginationObject>> {
    todo!()
}

#[get("/logs/<category>/stream")]
pub async fn stream<'a>(
    host: HostHeader<'a>,
    event_broker: State<'a, Arc<EventBroker>>,
    ws: State<'a, Arc<WebSocketServer>>,
    category: String,
) -> Result<WsStreamingResponder> {
    let id = OperationId(Uuid::new_v4().to_string());
    // TODO proper category -> topicname/tagvalue mapping
    let sub = event_broker
        .subscribe(TopicName(STDOUT_TOPIC_NAME.to_string()), move |tag_value| {
            tag_value == category
        })
        .await;

    let resp = WsStreamingResponder::new(Arc::clone(&ws), host, id);

    let ws = Arc::clone(&ws);
    let path = resp.path.clone();
    tokio::spawn(async move {
        ws.stream_at(path, sub, Duration::from_secs(300)).await;
    });

    Ok(resp)
}
