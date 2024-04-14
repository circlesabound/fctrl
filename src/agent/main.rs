#![feature(trait_alias)]

use std::{
    convert::{TryFrom, TryInto},
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};

use crate::{
    consts::*,
    factorio::{Factorio, VersionManager},
    server::{
        builder::{ServerBuilder, StartableInstanceBuilder},
        proc::ProcessManager,
        settings::{AdminList, LaunchSettings, ServerSettings},
        StoppedInstance,
    },
};
use chrono::Utc;
use factorio_mod_settings_parser::ModSettings;
use fctrl::schema::*;
use futures::Sink;
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use log::{debug, error, info, warn};
use server::{
    mods::{Mod, ModManager},
    settings::{BanList, Secrets, WhiteList},
};
use tokio::{
    fs,
    net::{TcpListener, TcpStream},
    sync::{
        broadcast::{self, error::RecvError},
        watch, Mutex, RwLock,
    },
    task::JoinHandle,
};
use tokio_tungstenite::{accept_async, tungstenite, WebSocketStream};
use tungstenite::Message;

mod consts;
mod error;
mod factorio;
mod server;
mod util;

const MAX_WS_PAYLOAD_BYTES: usize = 8000000;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    info!("Init Factorio installation manager");
    let version_manager = Arc::new(RwLock::new(
        VersionManager::new(&*FACTORIO_INSTALL_DIR).await?,
    ));

    info!("Init Factorio server process management");
    let proc_manager = Arc::new(ProcessManager::new());

    let (global_bus_tx, ..) = broadcast::channel::<AgentStreamingMessage>(300);

    info!("Init WebSocketListener");
    let ws_listener = WebSocketListener::new().await?;

    info!("Init SIGINT handler");
    let (sigint_tx, sigint_rx) = watch::channel(false);
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        info!("SIGINT detected");
        if let Err(e) = sigint_tx.send(true) {
            error!(
                "Failed to signal SIGINT channel, this will not be a clean shutdown. Error: {:?}",
                e
            );
        }
    });

    info!("Listening on {}", ws_listener.tcp.local_addr()?);
    ws_listener
        .run(
            sigint_rx,
            Arc::new(global_bus_tx),
            Arc::clone(&proc_manager),
            version_manager,
        )
        .await;

    info!("Shutting down");
    proc_manager.stop_instance().await;

    Ok(())
}

struct WebSocketListener {
    tcp: TcpListener,
}

impl WebSocketListener {
    async fn new() -> Result<WebSocketListener, std::io::Error> {
        // Safe to unwrap as this is checked by docker-compose
        let port = std::env::var(ENV_AGENT_WS_PORT).unwrap().parse().unwrap();
        let bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port);
        let tcp = TcpListener::bind(bind_addr).await?;
        Ok(WebSocketListener { tcp })
    }

    async fn run(
        self,
        mut shutdown_rx: watch::Receiver<bool>,
        global_bus_tx: Arc<broadcast::Sender<AgentStreamingMessage>>,
        proc_manager: Arc<ProcessManager>,
        version_manager: Arc<RwLock<VersionManager>>,
    ) {
        loop {
            tokio::select! {
                res = self.tcp.accept() => {
                    if let Ok((stream, _)) = res {
                        match AgentController::handle_connection(
                            stream,
                            shutdown_rx.clone(),
                            Arc::clone(&global_bus_tx),
                            Arc::clone(&proc_manager),
                            Arc::clone(&version_manager),
                        )
                        .await
                        {
                            Ok(controller) => {
                                tokio::spawn(async move {
                                    if let Err(e) = controller.message_loop().await {
                                        match e {
                                            tungstenite::Error::ConnectionClosed
                                            | tungstenite::Error::Protocol(_)
                                            | tungstenite::Error::Utf8 => (),
                                            err => error!("Error in message loop: {}", err),
                                        }
                                    }
                                });
                            }
                            Err(e) => match e {
                                tungstenite::Error::ConnectionClosed
                                | tungstenite::Error::Protocol(_)
                                | tungstenite::Error::Utf8 => (),
                                err => error!("Error handling connection: {}", err),
                            },
                        }
                    }
                }
                _ = shutdown_rx.changed() => {
                    break;
                }
            }
        }
    }
}

struct AgentController {
    peer_addr: SocketAddr,
    proc_manager: Arc<ProcessManager>,
    version_manager: Arc<RwLock<VersionManager>>,
    global_tx: Arc<broadcast::Sender<AgentStreamingMessage>>,
    ws_rx: Option<SplitStream<WebSocketStream<TcpStream>>>,
    ws_tx: Arc<Mutex<SplitSink<WebSocketStream<TcpStream>, Message>>>,
    _send_global_outgoing_msgs_task: JoinHandle<()>,
    _sigint_task: JoinHandle<()>,
}

impl AgentController {
    async fn handle_connection(
        tcp: TcpStream,
        mut shutdown_rx: watch::Receiver<bool>,
        global_bus_tx: Arc<broadcast::Sender<AgentStreamingMessage>>,
        proc_manager: Arc<ProcessManager>,
        version_manager: Arc<RwLock<VersionManager>>,
    ) -> tungstenite::Result<AgentController> {
        let peer_addr = tcp.peer_addr()?;
        let ws = accept_async(tcp).await?;
        let (ws_tx, ws_rx) = ws.split();
        let ws_tx = Arc::new(Mutex::new(ws_tx));
        info!("WebSocket peer connected: {}", peer_addr);

        // Set up background task to deliver outgoing messages from the global broadcast message bus
        let ws_tx_clone = Arc::clone(&ws_tx);
        let mut global_bus_rx = global_bus_tx.subscribe();
        let _send_global_outgoing_msgs_task = tokio::spawn(async move {
            loop {
                match global_bus_rx.recv().await {
                    Ok(outgoing) => {
                        let json = serde_json::to_string(&outgoing);
                        match json {
                            Err(e) => {
                                error!("Error serialising message: {:?}", e)
                            }
                            Ok(json) => {
                                debug!("Sending streaming message: {}", json);
                                AgentController::_send_message(
                                    Arc::clone(&ws_tx_clone),
                                    Message::Text(json),
                                )
                                .await;
                            }
                        }
                    }
                    Err(RecvError::Lagged(num_skipped)) => {
                        warn!("global bus rx lagging, skipped {} messages!", num_skipped)
                    }
                    Err(RecvError::Closed) => {
                        error!("All global bus senders closed - this should never happen");
                        break;
                    }
                }
            }

            warn!("Global bus rx listener exiting");
        });

        // Background task to close connection on SIGINT
        let ws_tx_close = Arc::clone(&ws_tx);
        let _sigint_task = tokio::spawn(async move {
            let _ = shutdown_rx.changed().await;
            info!("Closing WebSocket connection for peer {}", peer_addr);
            let _ = ws_tx_close.lock().await.close().await;
        });

        Ok(AgentController {
            peer_addr,
            proc_manager,
            version_manager,
            global_tx: global_bus_tx,
            ws_rx: Some(ws_rx),
            ws_tx,
            _send_global_outgoing_msgs_task,
            _sigint_task,
        })
    }

    async fn message_loop(mut self) -> tungstenite::Result<()> {
        let mut ws_rx = self.ws_rx.take().unwrap();
        let controller = Arc::new(self);
        while let Some(msg) = ws_rx.next().await {
            match msg {
                Ok(msg) => {
                    // handle close message here, to avoid passing values back and forth between tasks
                    if let Message::Close(_) = msg {
                        info!("Got close message from {}", controller.peer_addr);
                        break;
                    } else {
                        let controller = Arc::clone(&controller);
                        tokio::spawn(async move { controller.handle_message(msg).await });
                    }
                }
                Err(e) => {
                    error!("error with incoming message: {:?}", e);
                    if let tungstenite::Error::Io(io_error) = e {
                        if io_error.kind() == std::io::ErrorKind::BrokenPipe {
                            break;
                        }
                    }
                }
            }
        }

        info!("Cleaning up for peer {}", controller.peer_addr);
        controller._send_global_outgoing_msgs_task.abort();
        controller._sigint_task.abort();
        Ok(())
    }

    async fn handle_message(&self, msg: Message) {
        match msg {
            Message::Text(json) => {
                if let Ok(msg) = serde_json::from_str::<AgentRequestWithId>(&json) {
                    debug!("Got incoming message from {}: {}", self.peer_addr, json);
                    let operation_id = msg.operation_id;
                    match msg.message {
                        // ***********************
                        // Installation management
                        // ***********************
                        AgentRequest::VersionInstall {
                            version,
                            force_install,
                        } => {
                            self.version_install(version, force_install, operation_id)
                                .await
                        }

                        AgentRequest::VersionGet => {
                            self.version_get(operation_id).await;
                        }

                        // **************
                        // Server control
                        // **************
                        AgentRequest::ServerStart(savefile) => {
                            self.server_start(savefile, operation_id).await
                        }

                        AgentRequest::ServerStop => self.server_stop(operation_id).await,

                        AgentRequest::ServerStatus => self.server_status(operation_id).await,

                        // *******************
                        // Savefile management
                        // *******************
                        AgentRequest::SaveCreate(save_name) => {
                            self.save_create(save_name, operation_id).await
                        }

                        AgentRequest::SaveGet(save_name) => {
                            self.save_get(save_name, operation_id).await
                        }

                        AgentRequest::SaveList => {
                            self.save_list(operation_id).await;
                        }

                        AgentRequest::SaveSet(save_name, bytes) => {
                            todo!()
                        }

                        // **************
                        // Mod management
                        // **************
                        AgentRequest::ModListGet => {
                            self.mod_list_get(operation_id).await;
                        }

                        AgentRequest::ModListSet(mod_list) => {
                            self.mod_list_set(mod_list, operation_id).await;
                        }

                        AgentRequest::ModSettingsGet => {
                            self.mod_settings_get(operation_id).await;
                        }

                        AgentRequest::ModSettingsSet(bytes) => {
                            self.mod_settings_set(bytes, operation_id).await;
                        }

                        // *************
                        // Configuration
                        // *************
                        AgentRequest::ConfigAdminListGet => {
                            self.config_admin_list_get(operation_id).await;
                        }

                        AgentRequest::ConfigAdminListSet { admins } => {
                            self.config_admin_list_set(admins, operation_id).await;
                        }

                        AgentRequest::ConfigBanListGet => {
                            self.config_ban_list_get(operation_id).await;
                        }

                        AgentRequest::ConfigBanListSet { users } => {
                            self.config_ban_list_set(users, operation_id).await;
                        }

                        AgentRequest::ConfigRconGet => {
                            self.config_rcon_get(operation_id).await;
                        }

                        AgentRequest::ConfigRconSet { password } => {
                            self.config_rcon_set(password, operation_id).await;
                        }

                        AgentRequest::ConfigSecretsGet => {
                            self.config_secrets_get(operation_id).await;
                        }

                        AgentRequest::ConfigSecretsSet { username, token } => {
                            self.config_secrets_set(username, token, operation_id).await;
                        }

                        AgentRequest::ConfigServerSettingsGet => {
                            self.config_server_settings_get(operation_id).await;
                        }

                        AgentRequest::ConfigServerSettingsSet { config } => {
                            self.config_server_settings_set(config, operation_id).await;
                        }

                        AgentRequest::ConfigWhiteListGet => {
                            self.config_white_list_get(operation_id).await;
                        }

                        AgentRequest::ConfigWhiteListSet { enabled, users } => {
                            self.config_white_list_set(enabled, users, operation_id)
                                .await;
                        }

                        // *******
                        // In-game
                        // *******
                        AgentRequest::RconCommand(cmd) => {
                            self.rcon_command(cmd, operation_id).await
                        }
                    }
                }
            }
            Message::Binary(_) => {
                // binary messages not supported
            }
            Message::Ping(_) => {
                // tungstenite library handles pings already
            }
            Message::Close(_) => {
                // this should have been handled already
            }
            _ => {
                // other message
            }
        }
    }

    async fn _send_message<S: Sink<Message> + Unpin>(ws_tx: Arc<Mutex<S>>, message: Message)
    where
        <S as futures::Sink<tokio_tungstenite::tungstenite::Message>>::Error: std::fmt::Debug,
    {
        let mut tx = ws_tx.lock().await;
        if let Err(e) = tx.send(message).await {
            error!("Error sending message: {:?}", e);
        } else {
            let _ = tx.flush().await;
        }
    }

    async fn long_running_ack(&self, operation_id: &OperationId) {
        let with_id = AgentResponseWithId {
            operation_id: operation_id.clone(),
            status: OperationStatus::Ack,
            timestamp: Utc::now(),
            content: AgentOutMessage::Ok,
        };
        let json = serde_json::to_string(&with_id);
        match json {
            Err(e) => {
                error!("Error serialising message: {:?}", e);
            }
            Ok(json) => {
                debug!("Sending ack: {}", json);
                AgentController::_send_message(Arc::clone(&self.ws_tx), Message::Text(json)).await;
            }
        }
    }

    async fn reply(&self, message: AgentOutMessage, operation_id: &OperationId) {
        let with_id = AgentResponseWithId {
            operation_id: operation_id.clone(),
            status: OperationStatus::Ongoing,
            timestamp: Utc::now(),
            content: message,
        };
        let json = serde_json::to_string(&with_id);
        match json {
            Err(e) => {
                error!("Error serialising message: {:?}", e);
            }
            Ok(json) => {
                debug!("Sending reply: {}", json);
                AgentController::_send_message(Arc::clone(&self.ws_tx), Message::Text(json)).await;
            }
        }
    }

    async fn reply_success(&self, message: AgentOutMessage, operation_id: OperationId) {
        let with_id = AgentResponseWithId {
            operation_id,
            status: OperationStatus::Completed,
            timestamp: Utc::now(),
            content: message,
        };
        let json = serde_json::to_string(&with_id);
        match json {
            Err(e) => {
                error!("Error serialising message: {:?}", e);
            }
            Ok(json) => {
                debug!("Sending reply_success: {}", json);
                AgentController::_send_message(Arc::clone(&self.ws_tx), Message::Text(json)).await;
            }
        }
    }

    async fn reply_failed(&self, message: AgentOutMessage, operation_id: OperationId) {
        let with_id = AgentResponseWithId {
            operation_id,
            status: OperationStatus::Failed,
            timestamp: Utc::now(),
            content: message,
        };
        let json = serde_json::to_string(&with_id);
        match json {
            Err(e) => {
                error!("Error serialising message: {:?}", e);
            }
            Ok(json) => {
                debug!("Sending reply_failed: {}", json);
                AgentController::_send_message(Arc::clone(&self.ws_tx), Message::Text(json)).await;
            }
        }
    }

    async fn version_install(
        &self,
        version_to_install: FactorioVersion,
        force_install: bool,
        operation_id: OperationId,
    ) {
        if let Ok(mut vm) =
            tokio::time::timeout(Duration::from_millis(250), self.version_manager.write()).await
        {
            let version_to_install = version_to_install.0;
            self.long_running_ack(&operation_id).await;
            // Assume there is at most one version installed
            match vm.versions.keys().next() {
                None => {
                    info!("Installing version {}", version_to_install);
                    self.reply(
                        AgentOutMessage::Message(format!(
                            "Starting to install version {}",
                            version_to_install
                        )),
                        &operation_id,
                    )
                    .await;
                    if let Err(e) = vm.install(version_to_install.clone()).await {
                        self.reply_failed(
                            AgentOutMessage::Message(format!("Failed to install: {:?}", e)),
                            operation_id,
                        )
                        .await;
                    } else {
                        info!("Installed version {}", version_to_install);
                        self.reply_success(AgentOutMessage::Ok, operation_id).await;
                    }
                }
                Some(version_from) => {
                    let version_from = version_from.to_string();
                    let is_reinstall = version_from == version_to_install;

                    // Only reinstall if forced, otherwise noop
                    if is_reinstall && !force_install {
                        self.reply_success(AgentOutMessage::Ok, operation_id).await;
                        return;
                    }

                    let opt_stopped_instance;
                    if is_reinstall {
                        // Stop server first before re-installing
                        info!("Stopping server for reinstall");
                        opt_stopped_instance = self.proc_manager.stop_instance().await;
                        if opt_stopped_instance.is_some() {
                            self.reply(
                                AgentOutMessage::Message("Stopped server for reinstall".to_owned()),
                                &operation_id,
                            )
                            .await;
                        }

                        info!("Reinstalling version {}", version_to_install);
                        self.reply(
                            AgentOutMessage::Message(format!(
                                "Starting to reinstall version {}",
                                version_to_install
                            )),
                            &operation_id,
                        )
                        .await;
                        if let Err(e) = vm.install(version_to_install.clone()).await {
                            self.reply_failed(
                                AgentOutMessage::Error(format!("Failed to install: {:?}", e)),
                                operation_id,
                            )
                            .await;
                            return;
                        } else {
                            info!("Reinstalled version {}", version_to_install);
                            self.reply(
                                AgentOutMessage::Message(format!(
                                    "Reinstalled version {}",
                                    version_to_install
                                )),
                                &operation_id,
                            )
                            .await;
                        }
                    } else {
                        // Install requested version
                        info!("Installing version {} for upgrade", version_to_install);
                        self.reply(
                            AgentOutMessage::Message(format!(
                                "Starting to install version {}",
                                version_to_install
                            )),
                            &operation_id,
                        )
                        .await;
                        if let Err(e) = vm.install(version_to_install.clone()).await {
                            self.reply_failed(
                                AgentOutMessage::Error(format!("Failed to install: {:?}", e)),
                                operation_id,
                            )
                            .await;
                            return;
                        } else {
                            info!("Installed version {} for upgrade", version_to_install);
                            self.reply(
                                AgentOutMessage::Message(format!(
                                    "Installed version {} for upgrade",
                                    version_to_install
                                )),
                                &operation_id,
                            )
                            .await;
                        }

                        // Stop server if running
                        info!("Stopping server for upgrade");
                        opt_stopped_instance = self.proc_manager.stop_instance().await;
                        if opt_stopped_instance.is_some() {
                            self.reply(
                                AgentOutMessage::Message("Stopped server for upgrade".to_owned()),
                                &operation_id,
                            )
                            .await;
                        }
                    }

                    // TODO stage save migrations?

                    // If not a reinstall, remove previous version
                    if !is_reinstall {
                        info!("Removing previous version {} after upgrade", version_from);
                        if let Err(e) = vm.delete(&version_from).await {
                            self.reply_failed(AgentOutMessage::Error(format!("Failed to remove previous version {} after upgrading to version {}: {:?}", version_from, version_to_install, e)), operation_id).await;
                            return;
                        } else {
                            self.reply(
                                AgentOutMessage::Message(format!(
                                    "Removed previous version {} after upgrading to version {}",
                                    version_from, version_to_install
                                )),
                                &operation_id,
                            )
                            .await;
                        }
                    }

                    // Restart server if it was previously running
                    if let Some(previous_instance) = opt_stopped_instance {
                        info!("Restarting server");
                        self.reply(
                            AgentOutMessage::Message("Restarting server after upgrade".to_owned()),
                            &operation_id,
                        )
                        .await;
                        let version = vm.versions.get(&version_to_install).unwrap(); // safe since we still hold the lock
                        self.internal_server_start_with_version(
                            version,
                            previous_instance.savefile.clone(),
                            operation_id,
                            Some(previous_instance),
                        )
                        .await;
                    } else {
                        self.reply_success(AgentOutMessage::Ok, operation_id).await;
                    }
                }
            }
        } else {
            self.reply_failed(AgentOutMessage::ConflictingOperation, operation_id)
                .await;
        }
    }

    async fn version_get(&self, operation_id: OperationId) {
        if let Ok(vm) =
            tokio::time::timeout(Duration::from_millis(250), self.version_manager.read()).await
        {
            match vm.versions.values().next() {
                None => {
                    self.reply_failed(AgentOutMessage::NotInstalled, operation_id)
                        .await;
                }
                Some(v) => {
                    self.reply_success(
                        AgentOutMessage::FactorioVersion(v.version.clone().into()),
                        operation_id,
                    )
                    .await;
                }
            }
        } else {
            self.reply_failed(AgentOutMessage::ConflictingOperation, operation_id)
                .await;
        }
    }

    async fn server_start(&self, savefile: ServerStartSaveFile, operation_id: OperationId) {
        // assume there is at most one version installed
        if let Ok(vm) =
            tokio::time::timeout(Duration::from_millis(250), self.version_manager.read()).await
        {
            let version;
            match vm.versions.values().next() {
                None => {
                    self.reply_failed(AgentOutMessage::NotInstalled, operation_id)
                        .await;
                    return;
                }
                Some(v) => {
                    version = v;
                }
            }

            self.internal_server_start_with_version(version, savefile, operation_id, None)
                .await;
        } else {
            self.reply_failed(AgentOutMessage::ConflictingOperation, operation_id)
                .await;
        }
    }

    async fn internal_server_start_with_version(
        &self,
        version: &Factorio,
        savefile: ServerStartSaveFile,
        operation_id: OperationId,
        opt_restart_instance: Option<StoppedInstance>,
    ) {
        // Verify savefile exists
        if let ServerStartSaveFile::Specific(name) = &savefile {
            let save_path = util::saves::get_savefile_path(name);
            if !save_path.is_file() {
                self.reply_failed(
                    AgentOutMessage::Error(format!("Savefile with name {} does not exist", name)),
                    operation_id,
                )
                .await;
                return;
            }
        }

        // Latest save functionality doesn't work with custom save dir
        // Just disallow it
        if let ServerStartSaveFile::Latest = &savefile {
            self.reply_failed(
                AgentOutMessage::Error("Latest save functionality not implemented".to_owned()),
                operation_id,
            )
            .await;
            return;
        }

        // Mods
        let mods;
        match ModManager::read_or_apply_default().await {
            Ok(m) => mods = m,
            Err(_e) => {
                self.reply_failed(
                    AgentOutMessage::Error("Failed to read or initialise mod directory".to_owned()),
                    operation_id,
                )
                .await;
                return;
            }
        }

        // Launch settings is required to start
        // Pre-populate with default if not exist
        let launch_settings;
        match LaunchSettings::read_or_apply_default().await {
            Ok(ls) => launch_settings = ls,
            Err(_e) => {
                self.reply_failed(
                    AgentOutMessage::Error(
                        "Failed to read or initialise launch settings file".to_owned(),
                    ),
                    operation_id,
                )
                .await;
                return;
            }
        }

        // Server settings is required to start
        // Pre-populate with the example file if not exist
        let server_settings;
        match ServerSettings::read_or_apply_default(version).await {
            Ok(ss) => server_settings = ss,
            Err(_e) => {
                self.reply_failed(
                    AgentOutMessage::Error(
                        "Failed to read or initialise server settings file".to_owned(),
                    ),
                    operation_id,
                )
                .await;
                return;
            }
        }

        // Admin list
        let admin_list;
        match AdminList::read_or_apply_default().await {
            Ok(al) => admin_list = al,
            Err(_e) => {
                self.reply_failed(
                    AgentOutMessage::Error(
                        "Failed to read or initialise admin list file".to_owned(),
                    ),
                    operation_id,
                )
                .await;
                return;
            }
        }

        // Ban list
        let ban_list;
        match BanList::read_or_apply_default().await {
            Ok(bl) => ban_list = bl,
            Err(_e) => {
                self.reply_failed(
                    AgentOutMessage::Error("Failed to read or initialise ban list file".to_owned()),
                    operation_id,
                )
                .await;
                return;
            }
        }

        // White list
        let white_list;
        match WhiteList::read_or_apply_default().await {
            Ok(wl) => white_list = wl,
            Err(_e) => {
                self.reply_failed(
                    AgentOutMessage::Error(
                        "Failed to read or initialise white list file".to_owned(),
                    ),
                    operation_id,
                )
                .await;
                return;
            }
        }

        let stream_out = Arc::clone(&self.global_tx);
        let mut builder = ServerBuilder::using_installation(version)
            .with_stdout_handler(move |s| {
                let msg = AgentStreamingMessage {
                    timestamp: Utc::now(),
                    content: AgentStreamingMessageInner::ServerStdout(s),
                };
                if let Err(e) = stream_out.send(msg) {
                    error!("Failed to send streaming message: {:?}", e);
                }
            })
            .hosting_savefile(
                savefile,
                mods,
                admin_list,
                ban_list,
                white_list,
                launch_settings,
                server_settings,
            );

        if let Some(previous_instance) = opt_restart_instance {
            builder.replay_optional_args(previous_instance);
        }

        if let Err(e) = self.proc_manager.start_instance(builder).await {
            self.reply_failed(
                AgentOutMessage::Error(format!("Failed to start: {:?}", e)),
                operation_id,
            )
            .await;
        } else {
            self.reply_success(AgentOutMessage::Ok, operation_id).await;
        }
    }

    async fn server_stop(&self, operation_id: OperationId) {
        self.proc_manager.stop_instance().await;
        self.reply_success(AgentOutMessage::Ok, operation_id).await;
    }

    async fn server_status(&self, operation_id: OperationId) {
        let status = match self.proc_manager.status().await {
            server::proc::ProcessStatus::NotRunning => ServerStatus::NotRunning,
            server::proc::ProcessStatus::Running {
                server_state,
                player_count,
            } => match server_state {
                server::InternalServerState::Ready
                | server::InternalServerState::PreparedToHostGame
                | server::InternalServerState::CreatingGame => ServerStatus::PreGame,
                server::InternalServerState::InGame
                | server::InternalServerState::InGameSavingMap => {
                    ServerStatus::InGame { player_count }
                }
                server::InternalServerState::DisconnectingScheduled
                | server::InternalServerState::Disconnecting
                | server::InternalServerState::Disconnected
                | server::InternalServerState::Closed => ServerStatus::PostGame,
            },
        };
        self.reply_success(AgentOutMessage::ServerStatus(status), operation_id)
            .await;
    }

    async fn save_create(&self, save_name: String, operation_id: OperationId) {
        // assume there is at most one version installed
        if let Ok(version_mg) =
            tokio::time::timeout(Duration::from_millis(250), self.version_manager.read()).await
        {
            self.long_running_ack(&operation_id).await;
            let version;
            match version_mg.versions.values().next() {
                None => {
                    self.reply_failed(AgentOutMessage::NotInstalled, operation_id)
                        .await;
                    return;
                }
                Some(v) => {
                    version = v;
                }
            }

            // Create save dir if not exists
            let save_dir = &*SAVEFILE_DIR;
            if let Err(e) = fs::create_dir_all(save_dir).await {
                error!(
                    "Failed to create save dir at {}: {:?}",
                    save_dir.display(),
                    e
                );
                self.reply_failed(
                    AgentOutMessage::Error(format!("Failed to create save dir: {:?}", e)),
                    operation_id,
                )
                .await;
                return;
            }

            let stream_out = Arc::clone(&self.global_tx);
            let builder = ServerBuilder::using_installation(version)
                .with_stdout_handler(move |s| {
                    let msg = AgentStreamingMessage {
                        timestamp: Utc::now(),
                        content: AgentStreamingMessageInner::ServerStdout(s),
                    };
                    if let Err(e) = stream_out.send(msg) {
                        error!("Failed to send streaming message: {:?}", e);
                    }
                })
                .creating_savefile(save_name.clone());
            match self
                .proc_manager
                .start_and_wait_for_shortlived_instance(builder)
                .await
            {
                Err(e) => {
                    self.reply_failed(
                        AgentOutMessage::Error(format!("Savefile creation failed: {:?}", e)),
                        operation_id,
                    )
                    .await;
                }
                Ok(si) => {
                    if si.exit_status.success() {
                        self.reply_success(AgentOutMessage::Ok, operation_id).await;
                    } else {
                        self.reply_failed(
                            AgentOutMessage::Error(format!(
                                "Savefile creation failed: process exited with non-success code {}",
                                si.exit_status.to_string()
                            )),
                            operation_id,
                        )
                        .await;
                    }
                }
            }
        } else {
            self.reply_failed(AgentOutMessage::ConflictingOperation, operation_id)
                .await;
        }
    }

    async fn save_get(&self, save_name: String, operation_id: OperationId) {
        match util::saves::get_savefile(&save_name).await {
            Ok(Some(savebytes)) => {
                self.long_running_ack(&operation_id).await;
                let chunks = savebytes.bytes.chunks(MAX_WS_PAYLOAD_BYTES);
                let mut i = 0;
                for chunk in chunks {
                    let msg = AgentOutMessage::SaveFile(SaveBytes {
                        multipart_seqnum: Some(i),
                        bytes: chunk.to_vec(),
                    });
                    self.reply(msg, &operation_id).await;
                    i += 1;
                }
                self.reply_success(AgentOutMessage::SaveFile(SaveBytes::sentinel(i)), operation_id).await;
            },
            Ok(None) => self.reply_failed(
                AgentOutMessage::SaveNotFound,
                operation_id).await,
            Err(e) => self.reply_failed(
                AgentOutMessage::Error(format!("Failed to get save: {:?}", e)),
                operation_id).await,
        }
    }

    async fn save_list(&self, operation_id: OperationId) {
        match util::saves::list_savefiles().await {
            Ok(saves) => {
                self.reply_success(AgentOutMessage::SaveList(saves), operation_id)
                    .await;
            }
            Err(e) => {
                self.reply_failed(
                    AgentOutMessage::Error(format!("Failed to list saves: {:?}", e)),
                    operation_id,
                )
                .await;
            }
        }
    }

    async fn mod_list_get(&self, operation_id: OperationId) {
        match ModManager::read_or_apply_default().await {
            Ok(m) => {
                let list = m
                    .mods
                    .iter()
                    .map(|m| ModObject {
                        name: m.name.clone(),
                        version: m.version.clone(),
                    })
                    .collect();
                self.reply_success(AgentOutMessage::ModsList(list), operation_id)
                    .await;
            }
            Err(e) => {
                self.reply_failed(
                    AgentOutMessage::Error(format!("Failed to get mods: {:?}", e)),
                    operation_id,
                )
                .await;
            }
        }
    }

    async fn mod_list_set(&self, mod_list: Vec<ModObject>, operation_id: OperationId) {
        match ModManager::read_or_apply_default().await {
            Ok(mut m) => match Secrets::read().await {
                Ok(Some(s)) => {
                    m.mods = mod_list
                        .into_iter()
                        .map(|m| Mod {
                            name: m.name,
                            version: m.version,
                        })
                        .collect();
                    self.long_running_ack(&operation_id).await;
                    match m.apply(&s).await {
                        Ok(_) => {
                            self.reply_success(AgentOutMessage::Ok, operation_id).await;
                        }
                        Err(e) => {
                            self.reply_failed(
                                AgentOutMessage::Error(format!(
                                    "Failed to apply mod changes: {:?}",
                                    e
                                )),
                                operation_id,
                            )
                            .await;
                        }
                    }
                }
                Ok(None) => {
                    self.reply_failed(AgentOutMessage::MissingSecrets, operation_id)
                        .await;
                }
                Err(e) => {
                    self.reply_failed(
                        AgentOutMessage::Error(format!("Failed to read secrets: {:?}", e)),
                        operation_id,
                    )
                    .await;
                }
            },
            Err(e) => {
                self.reply_failed(
                    AgentOutMessage::Error(format!("Failed to initialise mod manager: {:?}", e)),
                    operation_id,
                )
                .await;
            }
        }
    }

    async fn mod_settings_get(&self, operation_id: OperationId) {
        match ModManager::read_or_apply_default().await {
            Ok(m) => {
                if let Some(s) = m.settings {
                    match s.try_into() {
                        Ok(bytes) => {
                            self.reply_success(
                                AgentOutMessage::ModSettings(Some(ModSettingsBytes { bytes })),
                                operation_id,
                            )
                            .await;
                        }
                        Err(e) => {
                            error!("Failed to serialise ModSettings: {:?}", e);
                            self.reply_failed(
                                AgentOutMessage::Error(format!(
                                    "Failed to parse ModSettings: {:?}",
                                    e
                                )),
                                operation_id,
                            )
                            .await;
                        }
                    }
                } else {
                    self.reply_success(AgentOutMessage::ModSettings(None), operation_id)
                        .await;
                }
            }
            Err(e) => {
                self.reply_failed(
                    AgentOutMessage::Error(format!("Failed to get mods: {:?}", e)),
                    operation_id,
                )
                .await;
            }
        }
    }

    async fn mod_settings_set(&self, ms_bytes: ModSettingsBytes, operation_id: OperationId) {
        match ModManager::read_or_apply_default().await {
            Ok(mut m) => {
                // Validate by attempting to parse
                match ModSettings::try_from(ms_bytes.bytes.as_ref()) {
                    Ok(ms) => {
                        m.settings = Some(ms);
                        if let Err(e) = m.apply_metadata_only().await {
                            self.reply_failed(
                                AgentOutMessage::Error(format!(
                                    "Unable to write mod settings: {:?}",
                                    e
                                )),
                                operation_id,
                            )
                            .await;
                        } else {
                            self.reply_success(AgentOutMessage::Ok, operation_id).await;
                        }
                    }
                    Err(e) => {
                        self.reply_failed(
                            AgentOutMessage::Error(format!(
                                "Unable to parse mod settings: {:?}",
                                e
                            )),
                            operation_id,
                        )
                        .await;
                    }
                }
            }
            Err(e) => {
                self.reply_failed(
                    AgentOutMessage::Error(format!("Failed to get mods: {:?}", e)),
                    operation_id,
                )
                .await;
            }
        }
    }

    async fn config_admin_list_get(&self, operation_id: OperationId) {
        match AdminList::read_or_apply_default().await {
            Ok(al) => {
                self.reply_success(AgentOutMessage::ConfigAdminList(al.list), operation_id)
                    .await;
            }
            Err(e) => {
                self.reply_failed(
                    AgentOutMessage::Error(format!(
                        "Failed to read or initialise admin list file: {:?}",
                        e
                    )),
                    operation_id,
                )
                .await;
            }
        }
    }

    async fn config_admin_list_set(&self, list: Vec<String>, operation_id: OperationId) {
        match AdminList::set(list).await {
            Ok(_) => {
                self.reply_success(AgentOutMessage::Ok, operation_id).await;
            }
            Err(e) => {
                self.reply_failed(
                    AgentOutMessage::Error(format!("Failed to set admin list: {:?}", e)),
                    operation_id,
                )
                .await;
            }
        }
    }

    async fn config_ban_list_get(&self, operation_id: OperationId) {
        match BanList::read_or_apply_default().await {
            Ok(bl) => {
                self.reply_success(AgentOutMessage::ConfigBanList(bl.list), operation_id)
                    .await;
            }
            Err(e) => {
                self.reply_failed(
                    AgentOutMessage::Error(format!(
                        "Failed to read or initialise ban list file: {:?}",
                        e
                    )),
                    operation_id,
                )
                .await;
            }
        }
    }

    async fn config_ban_list_set(&self, list: Vec<String>, operation_id: OperationId) {
        match BanList::set(list).await {
            Ok(_) => {
                self.reply_success(AgentOutMessage::Ok, operation_id).await;
            }
            Err(e) => {
                self.reply_failed(
                    AgentOutMessage::Error(format!("Failed to set ban list: {:?}", e)),
                    operation_id,
                )
                .await;
            }
        }
    }

    async fn config_rcon_get(&self, operation_id: OperationId) {
        match LaunchSettings::read_or_apply_default().await {
            Ok(ls) => {
                self.reply_success(
                    AgentOutMessage::ConfigRcon(RconConfig {
                        password: ls.rcon_password,
                        port: ls.rcon_bind.port(),
                    }),
                    operation_id,
                )
                .await;
            }
            Err(e) => {
                self.reply_failed(
                    AgentOutMessage::Error(format!(
                        "Failed to read or initialise launch settings file: {:?}",
                        e
                    )),
                    operation_id,
                )
                .await;
            }
        }
    }

    async fn config_rcon_set(&self, password: String, operation_id: OperationId) {
        match LaunchSettings::read_or_apply_default().await {
            Ok(mut ls) => {
                ls.rcon_password = password;
                if let Err(e) = ls.write().await {
                    self.reply_failed(
                        AgentOutMessage::Error(format!("Failed to set launch settings: {:?}", e)),
                        operation_id,
                    )
                    .await;
                } else {
                    self.reply_success(AgentOutMessage::Ok, operation_id).await;
                }
            }
            Err(e) => {
                self.reply_failed(
                    AgentOutMessage::Error(format!(
                        "Failed to read or initialise launch settings file: {:?}",
                        e
                    )),
                    operation_id,
                )
                .await;
            }
        }
    }

    async fn config_secrets_get(&self, operation_id: OperationId) {
        match Secrets::read().await {
            Ok(Some(s)) => {
                self.reply_success(
                    AgentOutMessage::ConfigSecrets(Some(SecretsObject {
                        username: s.username,
                        token: None,
                    })),
                    operation_id,
                )
                .await;
            }
            Ok(None) => {
                self.reply_success(AgentOutMessage::ConfigSecrets(None), operation_id)
                    .await;
            }
            Err(e) => {
                self.reply_failed(
                    AgentOutMessage::Error(format!("Failed to read secrets: {:?}", e)),
                    operation_id,
                )
                .await;
            }
        }
    }

    async fn config_secrets_set(&self, username: String, token: String, operation_id: OperationId) {
        let new = Secrets { username, token };
        match new.write().await {
            Ok(_) => {
                self.reply_success(AgentOutMessage::Ok, operation_id).await;
            }
            Err(e) => {
                self.reply_failed(
                    AgentOutMessage::Error(format!("Failed to set secrets: {:?}", e)),
                    operation_id,
                )
                .await;
            }
        }
    }

    async fn config_server_settings_get(&self, operation_id: OperationId) {
        if let Ok(Some(ss)) = ServerSettings::read().await {
            self.reply_success(AgentOutMessage::ConfigServerSettings(ss.config), operation_id)
                .await;
            return;
        }

        let vm = self.version_manager.read().await;
        if let Some((_, version)) = vm.versions.iter().next() {
            match ServerSettings::read_or_apply_default(version).await {
                Ok(ss) => {
                    self.reply_success(
                        AgentOutMessage::ConfigServerSettings(ss.config),
                        operation_id,
                    )
                    .await;
                }
                Err(e) => {
                    self.reply_failed(
                        AgentOutMessage::Error(format!(
                            "Failed to read or initialise server settings file: {:?}",
                            e
                        )),
                        operation_id,
                    )
                    .await;
                }
            }
        } else {
            self.reply_failed(AgentOutMessage::Error("No server settings saved and no version of Factorio is installed to generate a default".to_owned()), operation_id).await;
        }
    }

    async fn config_server_settings_set(&self, config: ServerSettingsConfig, operation_id: OperationId) {
        match ServerSettings::set(config).await {
            Ok(_) => {
                self.reply_success(AgentOutMessage::Ok, operation_id).await;
            }
            Err(e) => {
                self.reply_failed(
                    AgentOutMessage::Error(format!("Failed to set server settings: {:?}", e)),
                    operation_id,
                )
                .await;
            }
        }
    }

    async fn config_white_list_get(&self, operation_id: OperationId) {
        match LaunchSettings::read_or_apply_default().await {
            Ok(ls) => match WhiteList::read_or_apply_default().await {
                Ok(wl) => {
                    self.reply_success(
                        AgentOutMessage::ConfigWhiteList(WhitelistObject {
                            enabled: ls.use_whitelist,
                            users: wl.list,
                        }),
                        operation_id,
                    )
                    .await;
                }
                Err(e) => {
                    self.reply_failed(
                        AgentOutMessage::Error(format!(
                            "Failed to read or initialise white list file: {:?}",
                            e
                        )),
                        operation_id,
                    )
                    .await;
                }
            },
            Err(e) => {
                self.reply_failed(
                    AgentOutMessage::Error(format!(
                        "Failed to read or initialise launch settings file: {:?}",
                        e
                    )),
                    operation_id,
                )
                .await;
            }
        }
    }

    async fn config_white_list_set(
        &self,
        enabled: bool,
        list: Vec<String>,
        operation_id: OperationId,
    ) {
        match LaunchSettings::read_or_apply_default().await {
            Ok(mut ls) => {
                ls.use_whitelist = enabled;
                if let Err(e) = ls.write().await {
                    self.reply_failed(
                        AgentOutMessage::Error(format!("Failed to set launch settings: {:?}", e)),
                        operation_id,
                    )
                    .await;
                } else {
                    match WhiteList::set(list).await {
                        Ok(_) => {
                            self.reply_success(AgentOutMessage::Ok, operation_id).await;
                        }
                        Err(e) => {
                            self.reply_failed(
                                AgentOutMessage::Error(format!(
                                    "Failed to set white list: {:?}",
                                    e
                                )),
                                operation_id,
                            )
                            .await;
                        }
                    }
                }
            }
            Err(e) => {
                self.reply_failed(
                    AgentOutMessage::Error(format!(
                        "Failed to read or initialise launch settings file: {:?}",
                        e
                    )),
                    operation_id,
                )
                .await;
            }
        }
    }

    async fn rcon_command(&self, cmd: String, operation_id: OperationId) {
        match self.proc_manager.send_rcon_command_to_instance(&cmd).await {
            Ok(s) => {
                self.reply_success(AgentOutMessage::RconResponse(s), operation_id)
                    .await;
            }
            Err(e) => {
                error!("Couldn't send command to RCON: {:?}", e);
                self.reply_failed(
                    AgentOutMessage::Error(format!("Couldn't send command to RCON: {:?}", e)),
                    operation_id,
                )
                .await;
            }
        }
    }
}
