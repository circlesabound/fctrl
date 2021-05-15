use std::{sync::Arc, time::Duration};

use fctrl::schema::{mgmt_server_rest::LogsPaginationObject, OperationId};
use rocket::{get, State};
use rocket_contrib::json::Json;
use uuid::Uuid;

use crate::{
    db::{Cf, Db, RangeDirection},
    error::{Error, Result},
    events::{broker::EventBroker, TopicName, STDOUT_TOPIC_NAME},
    guards::HostHeader,
    routes::WsStreamingResponder,
    ws::WebSocketServer,
};

#[get("/logs/<category>?<count>&<direction>&<from>")]
pub async fn get<'a>(
    host: HostHeader<'a>,
    db: State<'a, Arc<Db>>,
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
    let next = ret.continue_from.map(|key| {
        format!(
            "{}/api/v0/logs/{}?count={}&direction={}&from={}",
            host.host, category, count, direction, key,
        )
    });

    let logs = ret.records.into_iter().map(|r| r.value).collect();

    Ok(Json(LogsPaginationObject { next, logs }))
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
