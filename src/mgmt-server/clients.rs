use std::{
    collections::HashMap,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, AtomicU8, Ordering},
        Arc,
    },
    time::Duration,
};

use fctrl::schema::{
    AgentOutMessage, AgentRequest, AgentRequestWithId, AgentResponseWithId, AgentStreamingMessage,
    OperationId, OperationStatus, Save, ServerStartSaveFile, ServerStatus,
};
use futures::{future, pin_mut, Future, SinkExt, Stream, StreamExt};
use log::{debug, error, info, warn};

use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

use crate::{
    error::{Error, Result},
    events::{broker::EventBroker, Event, TopicName, OPERATION_TOPIC_NAME, STDOUT_TOPIC_NAME},
};

pub struct AgentApiClient {
    event_broker: Arc<EventBroker>,
    ws_addr: url::Url,
    ws_connected: Arc<AtomicBool>,
}

impl AgentApiClient {
    pub async fn new(ws_addr: url::Url, event_broker: Arc<EventBroker>) -> AgentApiClient {
        let ws_connected = Arc::new(AtomicBool::new(false));

        let event_broker_clone = Arc::clone(&event_broker);
        let ws_addr_clone = ws_addr.clone();
        let ws_connected_clone = Arc::clone(&ws_connected);
        tokio::spawn(async move {
            loop {
                info!("Attempting to connect websocket");
                match connect(ws_addr_clone.clone(), Arc::clone(&event_broker_clone)).await {
                    Ok(dc_fut) => {
                        ws_connected_clone.store(true, Ordering::Relaxed);
                        dc_fut.await;
                        warn!("Websocket disconnected, will attempt to reconnect");
                        ws_connected_clone.store(false, Ordering::Relaxed);
                    }
                    Err(e) => {
                        error!("Failed to connect to websocket: {:?}", e);
                    }
                }

                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });

        AgentApiClient {
            event_broker,
            ws_addr,
            ws_connected,
        }
    }

    pub async fn version_install(
        &self,
        version: String,
        force_install: bool,
    ) -> Result<(OperationId, impl Stream<Item = Event>)> {
        let request = AgentRequest::VersionInstall {
            version,
            force_install,
        };
        let (id, mut sub) = self.send_request_and_subscribe(request).await?;

        let mut sub_pinned = Pin::new(&mut sub);
        match tokio::time::timeout(Duration::from_millis(500), sub_pinned.next()).await {
            Ok(Some(e)) => {
                let response_with_id = serde_json::from_str::<AgentResponseWithId>(&e.content)?;
                if let OperationStatus::Ack = response_with_id.status {
                    Ok((id, sub))
                } else {
                    // Long running operation should always respond with ack
                    Err(default_message_handler(response_with_id.content)
                        .unwrap_or(Error::AgentCommunicationError))
                }
            }
            Ok(None) => Err(Error::AgentDisconnected),
            Err(_) => Err(Error::AgentTimeout),
        }
    }

    pub async fn server_start(&self, savefile: ServerStartSaveFile) -> Result<()> {
        let request = AgentRequest::ServerStart(savefile);
        let (_id, sub) = self.send_request_and_subscribe(request).await?;

        pin_mut!(sub);
        loop {
            match tokio::time::timeout(Duration::from_millis(500), sub.next()).await {
                Ok(Some(e)) => {
                    let response_with_id = serde_json::from_str::<AgentResponseWithId>(&e.content)?;
                    match response_with_id.content {
                        AgentOutMessage::Ok => return Ok(()),
                        m => default_message_handler(m).map_or(Ok(()), |e| Err(e))?,
                    }
                }
                Ok(None) => return Err(Error::AgentDisconnected),
                Err(_) => return Err(Error::AgentTimeout),
            }
        }
    }

    pub async fn server_stop(&self) -> Result<()> {
        let request = AgentRequest::ServerStop;
        let (_id, sub) = self.send_request_and_subscribe(request).await?;

        pin_mut!(sub);
        loop {
            match tokio::time::timeout(Duration::from_millis(1000), sub.next()).await {
                Ok(Some(e)) => {
                    let response_with_id = serde_json::from_str::<AgentResponseWithId>(&e.content)?;
                    match response_with_id.content {
                        AgentOutMessage::Ok => return Ok(()),
                        m => default_message_handler(m).map_or(Ok(()), |e| Err(e))?,
                    }
                }
                Ok(None) => return Err(Error::AgentDisconnected),
                Err(_) => return Err(Error::AgentTimeout),
            }
        }
    }

    pub async fn server_status(&self) -> Result<ServerStatus> {
        let request = AgentRequest::ServerStatus;
        let (_id, sub) = self.send_request_and_subscribe(request).await?;

        pin_mut!(sub);
        loop {
            match tokio::time::timeout(Duration::from_millis(500), sub.next()).await {
                Ok(Some(e)) => {
                    let response_with_id = serde_json::from_str::<AgentResponseWithId>(&e.content)?;
                    match response_with_id.content {
                        AgentOutMessage::ServerStatus(s) => return Ok(s),
                        m => default_message_handler(m).map_or(Ok(()), |e| Err(e))?,
                    }
                }
                Ok(None) => return Err(Error::AgentDisconnected),
                Err(_) => return Err(Error::AgentTimeout),
            }
        }
    }

    pub async fn save_create(&self, savefile_name: String) -> Result<(OperationId, impl Stream<Item = Event> + Unpin)> {
        if savefile_name.trim().is_empty() {
            return Err(Error::BadRequest("Empty savefile name".to_owned()));
        }

        let request = AgentRequest::SaveCreate(savefile_name);
        let (id, mut sub) = self.send_request_and_subscribe(request).await?;

        let mut sub_pinned = Pin::new(&mut sub);
        match tokio::time::timeout(Duration::from_millis(500), sub_pinned.next()).await {
            Ok(Some(e)) => {
                let response_with_id = serde_json::from_str::<AgentResponseWithId>(&e.content)?;
                if let OperationStatus::Ack = response_with_id.status {
                    Ok((id, sub))
                } else {
                    // Long running operation should always respond with ack
                    Err(default_message_handler(response_with_id.content)
                        .unwrap_or(Error::AgentCommunicationError))
                }
            }
            Ok(None) => Err(Error::AgentDisconnected),
            Err(_) => Err(Error::AgentTimeout),
        }
    }

    pub async fn save_list(&self) -> Result<Vec<Save>> {
        let request = AgentRequest::SaveList;
        let (_id, sub) = self.send_request_and_subscribe(request).await?;

        pin_mut!(sub);
        loop {
            match tokio::time::timeout(Duration::from_millis(500), sub.next()).await {
                Ok(Some(e)) => {
                    let response_with_id = serde_json::from_str::<AgentResponseWithId>(&e.content)?;
                    match response_with_id.content {
                        AgentOutMessage::SaveList(saves) => return Ok(saves),
                        m => default_message_handler(m).map_or(Ok(()), |e| Err(e))?,
                    }
                }
                Ok(None) => return Err(Error::AgentDisconnected),
                Err(_) => return Err(Error::AgentTimeout),
            }
        }
    }

    async fn send_request_and_subscribe(
        &self,
        request: AgentRequest,
    ) -> Result<(OperationId, impl Stream<Item = Event> + Unpin)> {
        if !self.ws_connected.load(Ordering::Relaxed) {
            return Err(Error::AgentDisconnected);
        }

        let id = OperationId(Uuid::new_v4().to_string());
        let request_with_id = AgentRequestWithId {
            operation_id: id.clone(),
            message: request,
        };
        let mut tags = HashMap::new();
        tags.insert(
            TopicName(OUTGOING_TOPIC_NAME.to_owned()),
            self.ws_addr.to_string(),
        );
        let content = serde_json::to_string(&request_with_id)?;
        let event = Event { tags, content };

        let id_clone = id.clone();
        let subscriber = self
            .event_broker
            .subscribe(TopicName(OPERATION_TOPIC_NAME.to_owned()), move |v| {
                v == id_clone.0
            })
            .await;

        self.event_broker.publish(event).await;

        Ok((id, subscriber))
    }
}

/// "Default" handler for incoming messages from agent, to handle errors
fn default_message_handler(agent_message: AgentOutMessage) -> Option<Error> {
    match agent_message {
        AgentOutMessage::ConfigAdminList(_)
        | AgentOutMessage::ConfigRcon { .. }
        | AgentOutMessage::ConfigSecrets(_)
        | AgentOutMessage::ConfigServerSettings(_)
        | AgentOutMessage::FactorioVersion(_)
        | AgentOutMessage::ModsList(_)
        | AgentOutMessage::ModSettings(_)
        | AgentOutMessage::RconResponse(_)
        | AgentOutMessage::SaveList(_)
        | AgentOutMessage::ServerStatus(_)
        | AgentOutMessage::Ok => None,
        AgentOutMessage::Message(_) => {
            // swallow the message for now
            None
        }
        AgentOutMessage::Error(e) => Some(Error::AgentInternalError(e)),
        AgentOutMessage::ConflictingOperation => {
            Some(Error::AgentInternalError("Invalid operation at this time".to_owned()))
        }
        AgentOutMessage::MissingSecrets => {
            Some(Error::AgentInternalError("Missing secrets".to_owned()))
        }
        AgentOutMessage::NotInstalled => Some(Error::AgentInternalError(
            "Factorio not installed".to_owned(),
        )),
    }
}

const OUTGOING_TOPIC_NAME: &'static str = "_AGENT_OUTGOING";

/// Create a WebSocket connection and set it up to pipe incoming / outgoing to the event broker, using pub/sub.
/// This way we can easily re-create the connection at any time.
pub async fn connect(ws_addr: url::Url, event_broker: Arc<EventBroker>) -> Result<impl Future> {
    let (ws_stream, ..) = tokio_tungstenite::connect_async(&ws_addr).await?;
    let (ws_write, mut ws_read) = ws_stream.split();

    let outgoing_stream = event_broker
        .subscribe(TopicName(OUTGOING_TOPIC_NAME.to_owned()), move |s| {
            ws_addr.to_string() == s
        })
        .await;

    let ws_write = Arc::new(Mutex::new(ws_write));
    let ws_write_1 = Arc::clone(&ws_write);

    let consecutive_missed_pings = Arc::new(AtomicU8::new(0));
    let consecutive_missed_pings_1 = Arc::clone(&consecutive_missed_pings);
    let keep_alive_task = tokio::spawn(async move {
        while consecutive_missed_pings_1.load(Ordering::Relaxed) < 3 {
            tokio::time::sleep(Duration::from_secs(5)).await;
            let ping = Message::Ping(b"ping".to_vec());
            if let Err(e) = ws_write_1.lock().await.send(ping).await {
                error!("Failed to send ping: {:?}", e);
            } else {
                debug!("Sending keep-alive ping");
            }

            consecutive_missed_pings_1.fetch_add(1, Ordering::Relaxed);
        }
        warn!("Failed or missing 3 keep-alive pings, assuming dead connection.");
    });

    let ws_write_2 = Arc::clone(&ws_write);
    let forward_outgoing_task = tokio::spawn(async move {
        pin_mut!(outgoing_stream);
        while let Some(outgoing_event) = outgoing_stream.next().await {
            let msg = Message::Text(outgoing_event.content);
            if let Err(e) = ws_write_2.lock().await.send(msg).await {
                error!("Websocket error sending request to agent: {:?}", e);
                break;
            }
        }

        warn!("forward_outgoing_task exiting, due to pipe disconnection");
    });

    let publish_incoming_task = tokio::spawn(async move {
        while let Some(incoming) = ws_read.next().await {
            match incoming {
                Ok(msg) => {
                    match msg {
                        Message::Text(s) => {
                            if let Some(event) = tag_incoming_message(s) {
                                event_broker.publish(event).await;
                            }
                        }
                        Message::Binary(_) => {
                            warn!("Got binary message, not supported");
                        }
                        Message::Ping(ping) => {
                            info!("Agent sent ping, responding with pong");
                            let pong = Message::Pong(ping);
                            if let Err(e) = ws_write.lock().await.send(pong).await {
                                error!("Failed to respond with pong: {:?}", e);
                            }
                        }
                        Message::Pong(_) => {
                            // Reset the keepalive
                            debug!("Received pong response, resetting keepalive");
                            consecutive_missed_pings.fetch_min(0, Ordering::Relaxed);
                        }
                        Message::Close(_) => {
                            warn!("Agent requested to close the websocket connection");
                            let _ = ws_write.lock().await.close().await;
                            break;
                        }
                    }
                }
                Err(e) => {
                    error!("Websocket error receiving messages from agent: {:?}", e);
                    break;
                }
            }
        }

        warn!("publish_incoming_task exiting, due to pipe disconnection");
    });

    let disconnect_tasks = vec![
        keep_alive_task,
        forward_outgoing_task,
        publish_incoming_task,
    ];
    let fut_disconnect = future::select_all(disconnect_tasks);

    Ok(fut_disconnect)
}

fn tag_incoming_message(s: String) -> Option<Event> {
    if let Ok(response_with_id) = serde_json::from_str::<AgentResponseWithId>(&s) {
        let mut tags = HashMap::new();
        tags.insert(
            TopicName(OPERATION_TOPIC_NAME.to_owned()),
            response_with_id.operation_id.into(),
        );
        let event = Event { tags, content: s };
        Some(event)
    } else if let Ok(streaming_msg) = serde_json::from_str::<AgentStreamingMessage>(&s) {
        let mut tags = HashMap::new();
        match streaming_msg {
            AgentStreamingMessage::ServerStdout(_) => {
                tags.insert(TopicName(STDOUT_TOPIC_NAME.to_owned()), String::new());
            }
        }
        let event = Event { tags, content: s };
        Some(event)
    } else {
        warn!("Got text message of unsupported format: {}", s);
        None
    }
}
