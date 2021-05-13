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
    FactorioVersion, ModObject, ModSettingsBytes, OperationId, OperationStatus, RconConfig, Save,
    SecretsObject, ServerStartSaveFile, ServerStatus, WhitelistObject,
};
use futures::{future, pin_mut, Future, SinkExt, Stream, StreamExt};
use lazy_static::lazy_static;
use log::{error, info, trace, warn};
use regex::Regex;
use stream_cancel::Valved;
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

use crate::{consts, error::{Error, Result}, events::{Event, OPERATION_TOPIC_NAME, STDOUT_TOPIC_CHAT_CATEGORY, STDOUT_TOPIC_JOINLEAVE_CATEGORY, STDOUT_TOPIC_NAME, STDOUT_TOPIC_SYSTEMLOG_CATEGORY, TopicName, broker::EventBroker}};

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
                info!("Attempting to establish WebSocket connection with agent");
                match connect(ws_addr_clone.clone(), Arc::clone(&event_broker_clone)).await {
                    Ok(dc_fut) => {
                        ws_connected_clone.store(true, Ordering::Relaxed);
                        dc_fut.await;
                        warn!("Agent WebSocket disconnected, will attempt to reconnect");
                        ws_connected_clone.store(false, Ordering::Relaxed);
                    }
                    Err(e) => {
                        error!("Failed to connect to agent websocket: {:?}", e);
                    }
                }

                // Delay 3 seconds before reconnecting
                tokio::time::sleep(Duration::from_secs(3)).await;
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
        version: FactorioVersion,
        force_install: bool,
    ) -> Result<(OperationId, impl Stream<Item = Event>)> {
        let request = AgentRequest::VersionInstall {
            version,
            force_install,
        };
        let (id, sub) = self.send_request_and_subscribe(request).await?;

        ack_or_timeout(sub, Duration::from_millis(500), id).await
    }

    pub async fn version_get(&self) -> Result<FactorioVersion> {
        let request = AgentRequest::VersionGet;
        let (_id, sub) = self.send_request_and_subscribe(request).await?;

        response_or_timeout(sub, Duration::from_millis(500), |r| match r.content {
            AgentOutMessage::FactorioVersion(v) => Ok(v),
            m => Err(default_message_handler(m)),
        })
        .await
    }

    pub async fn server_start(&self, savefile: ServerStartSaveFile) -> Result<()> {
        let request = AgentRequest::ServerStart(savefile);
        let (_id, sub) = self.send_request_and_subscribe(request).await?;

        response_or_timeout(sub, Duration::from_millis(2000), |r| match r.content {
            AgentOutMessage::Ok => Ok(()),
            m => Err(default_message_handler(m)),
        })
        .await
    }

    pub async fn server_stop(&self) -> Result<()> {
        let request = AgentRequest::ServerStop;
        let (_id, sub) = self.send_request_and_subscribe(request).await?;

        response_or_timeout(sub, Duration::from_millis(2000), |r| match r.content {
            AgentOutMessage::Ok => Ok(()),
            m => Err(default_message_handler(m)),
        })
        .await
    }

    pub async fn server_status(&self) -> Result<ServerStatus> {
        let request = AgentRequest::ServerStatus;
        let (_id, sub) = self.send_request_and_subscribe(request).await?;

        response_or_timeout(sub, Duration::from_millis(500), |r| match r.content {
            AgentOutMessage::ServerStatus(s) => Ok(s),
            m => Err(default_message_handler(m)),
        })
        .await
    }

    pub async fn save_create(
        &self,
        savefile_name: String,
    ) -> Result<(OperationId, impl Stream<Item = Event> + Unpin)> {
        if savefile_name.trim().is_empty() {
            return Err(Error::BadRequest("Empty savefile name".to_owned()));
        }

        let request = AgentRequest::SaveCreate(savefile_name);
        let (id, sub) = self.send_request_and_subscribe(request).await?;

        ack_or_timeout(sub, Duration::from_millis(500), id).await
    }

    pub async fn save_list(&self) -> Result<Vec<Save>> {
        let request = AgentRequest::SaveList;
        let (_id, sub) = self.send_request_and_subscribe(request).await?;

        response_or_timeout(sub, Duration::from_millis(500), |r| match r.content {
            AgentOutMessage::SaveList(saves) => Ok(saves),
            m => Err(default_message_handler(m)),
        })
        .await
    }

    pub async fn mod_list_get(&self) -> Result<Vec<ModObject>> {
        let request = AgentRequest::ModListGet;
        let (_id, sub) = self.send_request_and_subscribe(request).await?;

        response_or_timeout(sub, Duration::from_millis(500), |r| match r.content {
            AgentOutMessage::ModsList(mods) => Ok(mods),
            m => Err(default_message_handler(m)),
        })
        .await
    }

    pub async fn mod_list_set(
        &self,
        mods: Vec<ModObject>,
    ) -> Result<(OperationId, impl Stream<Item = Event> + Unpin)> {
        let request = AgentRequest::ModListSet(mods);
        let (id, sub) = self.send_request_and_subscribe(request).await?;

        ack_or_timeout(sub, Duration::from_millis(500), id).await
    }

    pub async fn mod_settings_get(&self) -> Result<ModSettingsBytes> {
        let request = AgentRequest::ModSettingsGet;
        let (_id, sub) = self.send_request_and_subscribe(request).await?;

        response_or_timeout(sub, Duration::from_millis(500), |r| match r.content {
            AgentOutMessage::ModSettings(Some(mod_settings)) => Ok(mod_settings),
            AgentOutMessage::ModSettings(None) => Err(Error::ModSettingsNotInitialised),
            m => Err(default_message_handler(m)),
        })
        .await
    }

    pub async fn mod_settings_set(&self, mod_settings: ModSettingsBytes) -> Result<()> {
        let request = AgentRequest::ModSettingsSet(mod_settings);
        let (_id, sub) = self.send_request_and_subscribe(request).await?;

        response_or_timeout(sub, Duration::from_millis(500), |r| match r.content {
            AgentOutMessage::Ok => Ok(()),
            m => Err(default_message_handler(m)),
        })
        .await
    }

    pub async fn config_adminlist_get(&self) -> Result<Vec<String>> {
        let request = AgentRequest::ConfigAdminListGet;
        let (_id, sub) = self.send_request_and_subscribe(request).await?;

        response_or_timeout(sub, Duration::from_millis(500), |r| match r.content {
            AgentOutMessage::ConfigAdminList(admin_list) => Ok(admin_list),
            m => Err(default_message_handler(m)),
        })
        .await
    }

    pub async fn config_adminlist_set(&self, admins: Vec<String>) -> Result<()> {
        let request = AgentRequest::ConfigAdminListSet { admins };
        let (_id, sub) = self.send_request_and_subscribe(request).await?;

        response_or_timeout(sub, Duration::from_millis(500), |r| match r.content {
            AgentOutMessage::Ok => Ok(()),
            m => Err(default_message_handler(m)),
        })
        .await
    }

    pub async fn config_banlist_get(&self) -> Result<Vec<String>> {
        let request = AgentRequest::ConfigBanListGet;
        let (_id, sub) = self.send_request_and_subscribe(request).await?;

        response_or_timeout(sub, Duration::from_millis(500), |r| match r.content {
            AgentOutMessage::ConfigBanList(ban_list) => Ok(ban_list),
            m => Err(default_message_handler(m)),
        })
        .await
    }

    pub async fn config_banlist_set(&self, users: Vec<String>) -> Result<()> {
        let request = AgentRequest::ConfigBanListSet { users };
        let (_id, sub) = self.send_request_and_subscribe(request).await?;

        response_or_timeout(sub, Duration::from_millis(500), |r| match r.content {
            AgentOutMessage::Ok => Ok(()),
            m => Err(default_message_handler(m)),
        })
        .await
    }

    pub async fn config_rcon_get(&self) -> Result<RconConfig> {
        let request = AgentRequest::ConfigRconGet;
        let (_id, sub) = self.send_request_and_subscribe(request).await?;

        response_or_timeout(sub, Duration::from_millis(500), |r| match r.content {
            AgentOutMessage::ConfigRcon(rcon_config) => Ok(rcon_config),
            m => Err(default_message_handler(m)),
        })
        .await
    }

    pub async fn config_rcon_set(&self, rcon_config: RconConfig) -> Result<()> {
        // ignore port because it is read only
        let request = AgentRequest::ConfigRconSet {
            password: rcon_config.password,
        };
        let (_id, sub) = self.send_request_and_subscribe(request).await?;

        response_or_timeout(sub, Duration::from_millis(500), |r| match r.content {
            AgentOutMessage::Ok => Ok(()),
            m => Err(default_message_handler(m)),
        })
        .await
    }

    pub async fn config_secrets_get(&self) -> Result<SecretsObject> {
        let request = AgentRequest::ConfigSecretsGet;
        let (_id, sub) = self.send_request_and_subscribe(request).await?;

        response_or_timeout(sub, Duration::from_millis(500), |r| match r.content {
            AgentOutMessage::ConfigSecrets(Some(secrets)) => Ok(SecretsObject {
                username: secrets.username,
                token: None,
            }),
            AgentOutMessage::ConfigSecrets(None) => Err(Error::SecretsNotInitialised),
            m => Err(default_message_handler(m)),
        })
        .await
    }

    pub async fn config_secrets_set(&self, secrets: SecretsObject) -> Result<()> {
        let request = AgentRequest::ConfigSecretsSet {
            username: secrets.username,
            token: secrets.token.unwrap_or_default(),
        };
        let (_id, sub) = self.send_request_and_subscribe(request).await?;

        response_or_timeout(sub, Duration::from_millis(500), |r| match r.content {
            AgentOutMessage::Ok => Ok(()),
            m => Err(default_message_handler(m)),
        })
        .await
    }

    pub async fn config_server_settings_get(&self) -> Result<String> {
        let request = AgentRequest::ConfigServerSettingsGet;
        let (_id, sub) = self.send_request_and_subscribe(request).await?;

        response_or_timeout(sub, Duration::from_millis(500), |r| match r.content {
            AgentOutMessage::ConfigServerSettings(json) => Ok(json),
            m => Err(default_message_handler(m)),
        })
        .await
    }

    pub async fn config_server_settings_set(&self, json: String) -> Result<()> {
        let request = AgentRequest::ConfigServerSettingsSet { json };
        let (_id, sub) = self.send_request_and_subscribe(request).await?;

        response_or_timeout(sub, Duration::from_millis(500), |r| match r.content {
            AgentOutMessage::Ok => Ok(()),
            m => Err(default_message_handler(m)),
        })
        .await
    }

    pub async fn config_whitelist_get(&self) -> Result<WhitelistObject> {
        let request = AgentRequest::ConfigWhiteListGet;
        let (_id, sub) = self.send_request_and_subscribe(request).await?;

        response_or_timeout(sub, Duration::from_millis(500), |r| match r.content {
            AgentOutMessage::ConfigWhiteList(wl) => Ok(wl),
            m => Err(default_message_handler(m)),
        })
        .await
    }

    pub async fn config_whitelist_set(&self, enabled: bool, users: Vec<String>) -> Result<()> {
        let request = AgentRequest::ConfigWhiteListSet { enabled, users };
        let (_id, sub) = self.send_request_and_subscribe(request).await?;

        response_or_timeout(sub, Duration::from_millis(500), |r| match r.content {
            AgentOutMessage::Ok => Ok(()),
            m => Err(default_message_handler(m)),
        })
        .await
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
fn default_message_handler(agent_message: AgentOutMessage) -> Error {
    match agent_message {
        AgentOutMessage::ConfigAdminList(_)
        | AgentOutMessage::ConfigBanList(_)
        | AgentOutMessage::ConfigRcon { .. }
        | AgentOutMessage::ConfigSecrets(_)
        | AgentOutMessage::ConfigServerSettings(_)
        | AgentOutMessage::ConfigWhiteList(_)
        | AgentOutMessage::FactorioVersion(_)
        | AgentOutMessage::Message(_)
        | AgentOutMessage::ModsList(_)
        | AgentOutMessage::ModSettings(_)
        | AgentOutMessage::RconResponse(_)
        | AgentOutMessage::SaveList(_)
        | AgentOutMessage::ServerStatus(_)
        | AgentOutMessage::Ok => Error::AgentCommunicationError,
        AgentOutMessage::Error(e) => Error::AgentInternalError(e),
        AgentOutMessage::ConflictingOperation => {
            Error::AgentInternalError("Invalid operation at this time".to_owned())
        }
        AgentOutMessage::MissingSecrets => Error::AgentInternalError("Missing secrets".to_owned()),
        AgentOutMessage::NotInstalled => {
            Error::AgentInternalError("Factorio not installed".to_owned())
        }
    }
}

const OUTGOING_TOPIC_NAME: &str = "_AGENT_OUTGOING";

/// Create a WebSocket connection and set it up to pipe incoming / outgoing to the event broker, using pub/sub.
/// This way we can easily re-create the connection at any time.
pub async fn connect(ws_addr: url::Url, event_broker: Arc<EventBroker>) -> Result<impl Future> {
    let (ws_stream, ..) = tokio_tungstenite::connect_async(&ws_addr).await?;
    info!("Agent WebSocket connected");
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
        while consecutive_missed_pings_1.load(Ordering::Acquire) < 3 {
            tokio::time::sleep(Duration::from_secs(15)).await;
            let ping = Message::Ping(b"ping".to_vec());
            if let Err(e) = ws_write_1.lock().await.send(ping).await {
                error!("Failed to send ping: {:?}", e);
            } else {
                trace!("Sending keep-alive ping");
            }

            consecutive_missed_pings_1.fetch_add(1, Ordering::AcqRel);
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
                            // binary message not supported or used
                        }
                        Message::Ping(_) => {
                            // tungstenite library handles pings already
                        }
                        Message::Pong(_) => {
                            // Reset the keepalive
                            trace!("Received pong response, resetting keepalive");
                            consecutive_missed_pings.fetch_min(0, Ordering::Release);
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
            TopicName(OPERATION_TOPIC_NAME.to_string()),
            response_with_id.operation_id.into(),
        );
        let event = Event { tags, content: s };
        Some(event)
    } else if let Ok(streaming_msg) = serde_json::from_str::<AgentStreamingMessage>(&s) {
        let mut tags = HashMap::new();
        match streaming_msg {
            AgentStreamingMessage::ServerStdout(stdout_message) => {
                let topic = TopicName(STDOUT_TOPIC_NAME.to_string());
                let tag_value = classify_server_stdout_message(&stdout_message);
                tags.insert(TopicName(STDOUT_TOPIC_NAME.to_string()), tag_value);
            }
        }
        let event = Event { tags, content: s };
        Some(event)
    } else {
        warn!("Got text message of unsupported format: {}", s);
        None
    }
}

async fn response_or_timeout<HandlerFn, T>(
    sub: impl Stream<Item = Event> + Unpin,
    timeout: Duration,
    response_handler: HandlerFn,
) -> Result<T>
where
    T: Sized,
    HandlerFn: FnOnce(AgentResponseWithId) -> Result<T> + Sized,
{
    pin_mut!(sub);
    match tokio::time::timeout(timeout, sub.next()).await {
        Ok(Some(e)) => {
            let response_with_id = serde_json::from_str::<AgentResponseWithId>(&e.content)?;
            response_handler(response_with_id)
        }
        Ok(None) => Err(Error::AgentDisconnected),
        Err(_) => Err(Error::AgentTimeout),
    }
}

async fn ack_or_timeout(
    mut sub: impl Stream<Item = Event> + Unpin,
    no_ack_timeout: Duration,
    operation_id: OperationId,
) -> Result<(OperationId, impl Stream<Item = Event> + Unpin)> {
    let mut sub_pinned = Pin::new(&mut sub);
    match tokio::time::timeout(no_ack_timeout, sub_pinned.next()).await {
        Ok(Some(e)) => {
            let response_with_id = serde_json::from_str::<AgentResponseWithId>(&e.content)?;
            if let OperationStatus::Ack = response_with_id.status {
                Ok((operation_id, fuse_agent_response_stream(sub)))
            } else {
                // Long running operation should always respond with ack
                Err(default_message_handler(response_with_id.content))
            }
        }
        Ok(None) => Err(Error::AgentDisconnected),
        Err(_) => Err(Error::AgentTimeout),
    }
}

#[derive(Debug)]
enum StreamSignal {
    Close,
}

/// Fuse (close) the stream after a AgentResponseWithId has status Completed or Failed.
fn fuse_agent_response_stream(s: impl Stream<Item = Event>) -> impl Stream<Item = Event> {
    let (tx, mut rx) = mpsc::channel(1);
    let (exit, valved) = Valved::new(s);

    tokio::spawn(async move {
        let _ = rx.recv().await;
        exit.cancel();
    });

    let tx = Arc::new(tx);
    valved.inspect(move |e| {
        match serde_json::from_str::<AgentResponseWithId>(&e.content) {
            Ok(r) => match r.status {
                OperationStatus::Ack | OperationStatus::Ongoing => (),
                OperationStatus::Completed | OperationStatus::Failed => {
                    if let Err(e) = tx.try_send(StreamSignal::Close) {
                        error!("error signalling response stream end: {:?}", e);
                    }
                }
            },
            Err(_) => {
                warn!("Failed to deserialise AgentResponseWithId");
                // ignore
            }
        }
    })
}

fn classify_server_stdout_message(message: &str) -> String {
    lazy_static! {
        static ref CHAT_RE: Regex =
            Regex::new(r"^(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}) \[CHAT\] ([^:]+): (.+)$").unwrap();
        static ref JOIN_RE: Regex = Regex::new(
            r"^(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}) \[JOIN\] ([^:]+): joined the game$"
        )
        .unwrap();
        static ref LEAVE_RE: Regex =
            Regex::new(r"^(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}) \[LEAVE\] ([^:]+) left the game$")
                .unwrap();
    }

    if let Some(chat_captures) = CHAT_RE.captures(message) {
        let timestamp = chat_captures.get(1).unwrap().as_str().to_string();
        let user = chat_captures.get(2).unwrap().as_str().to_string();
        let msg = chat_captures.get(3).unwrap().as_str().to_string();
        STDOUT_TOPIC_CHAT_CATEGORY.to_string()
    } else if let Some(join_captures) = JOIN_RE.captures(message) {
        let timestamp = join_captures.get(1).unwrap().as_str().to_string();
        let user = join_captures.get(2).unwrap().as_str().to_string();
        STDOUT_TOPIC_JOINLEAVE_CATEGORY.to_string()
    } else if let Some(leave_captures) = LEAVE_RE.captures(message) {
        let timestamp = leave_captures.get(1).unwrap().as_str().to_string();
        let user = leave_captures.get(2).unwrap().as_str().to_string();
        STDOUT_TOPIC_JOINLEAVE_CATEGORY.to_string()
    } else {
        STDOUT_TOPIC_SYSTEMLOG_CATEGORY.to_string()
    }
}
