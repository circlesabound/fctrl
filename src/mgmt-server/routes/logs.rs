use std::{sync::Arc, time::Duration};

use fctrl::schema::{
    mgmt_server_rest::{LogStreamPreviousMarker, LogsPaginationObject},
    OperationId,
};
use rocket::{get, serde::json::Json, State};
use uuid::Uuid;

use crate::{
    db::{Cf, Db, RangeDirection},
    error::{Error, Result},
    events::{broker::EventBroker, TopicName, STDOUT_TOPIC_NAME},
    guards::HostHeader,
    ws::WebSocketServer,
};

use super::WsStreamingResponderWithPreviousMarker;

#[get("/logs/<category>?<count>&<direction>&<from>")]
pub async fn get<'a>(
    // host: HostHeader<'a>,
    db: &State<Arc<Db>>,
    category: String,
    count: u32,
    direction: String,
    from: Option<String>,
) -> Result<Json<LogsPaginationObject>> {
    let cf = Cf(category.clone());

    let range_direction = match direction.to_lowercase().as_ref() {
        "forward" => Ok(RangeDirection::Forward),
        "backward" => Ok(RangeDirection::Backward),
        s => Err(Error::BadRequest(format!(
            "Invalid direction '{}', expected Forward or Backward",
            s
        ))),
    }?;

    let ret;
    if let Some(from_key) = from {
        ret = db.read_range(&cf, from_key, range_direction, count)?;
    } else {
        ret = match range_direction {
            RangeDirection::Forward => db.read_range_head(&cf, count)?,
            RangeDirection::Backward => db.read_range_tail(&cf, count)?,
        };
    }

    // Calculate url for next
    let next = ret.continue_from;
    let logs = ret.records.into_iter().map(|r| r.value).collect();

    Ok(Json(LogsPaginationObject { next, logs }))
}

#[get("/logs/<category>/stream")]
pub async fn stream<'a>(
    host: HostHeader<'a>,
    db: &State<Arc<Db>>,
    event_broker: &State<Arc<EventBroker>>,
    ws: &State<Arc<WebSocketServer>>,
    category: String,
) -> Result<WsStreamingResponderWithPreviousMarker> {
    let id = OperationId(Uuid::new_v4().to_string());

    // Get the previous marker from DB
    let cf = Cf(category.clone());
    let ret = db.read_range_tail(&cf, 1)?;
    let previous = ret.records.get(0).map(|r| r.key.clone());

    // TODO proper category -> topicname/tagvalue mapping
    let sub = event_broker
        .subscribe(TopicName(STDOUT_TOPIC_NAME.to_string()), move |tag_value| {
            tag_value == category
        })
        .await;

    let resp = WsStreamingResponderWithPreviousMarker::new(
        Arc::clone(&ws),
        host,
        id,
        LogStreamPreviousMarker { previous },
    );

    let ws = Arc::clone(&ws);
    let path = resp.base.path.clone();
    tokio::spawn(async move {
        ws.stream_at(path, sub, Duration::from_secs(300)).await;
    });

    Ok(resp)
}
