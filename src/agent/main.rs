#![feature(trait_alias)]

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
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
use fctrl::schema::*;
use futures::Sink;
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use log::{debug, error, info, warn};
// use logwatcher::LogWatcher;
use rcon::Connection;
use server::mods::ModManager;
use tokio::{
    fs,
    net::{TcpListener, TcpStream},
    sync::{broadcast, watch, Mutex},
    task::JoinHandle,
};
use tokio_tungstenite::{accept_async, tungstenite, WebSocketStream};
use tungstenite::Message;

mod consts;
mod error;
mod factorio;
mod server;
mod util;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    info!("Init Factorio installation manager");
    let version_manager = Arc::new(Mutex::new(
        VersionManager::new(&*FACTORIO_INSTALL_DIR).await?,
    ));

    info!("Init Factorio server process management");
    let proc_manager = Arc::new(ProcessManager::new());

    // TODO rework rcon, likely include it as part of factorio instance
    let rcon = None;
    // info!("Init RCON");
    // let rcon;
    // if let Some(rcon_config) = config.rcon {
    //     rcon = Some(rcon_connect(&rcon_config).await?);
    // } else {
    //     warn!("No RCON connection established as config section is missing");
    //     rcon = None;
    // }

    let (global_bus_tx, ..) = broadcast::channel::<AgentStreamingMessage>(50);

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
            Arc::new(Mutex::new(rcon)),
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
        rcon: Arc<Mutex<Option<Connection>>>,
        version_manager: Arc<Mutex<VersionManager>>,
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
                            Arc::clone(&rcon),
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
    rcon: Arc<Mutex<Option<Connection>>>,
    version_manager: Arc<Mutex<VersionManager>>,
    global_tx: Arc<broadcast::Sender<AgentStreamingMessage>>,
    ws_rx: SplitStream<WebSocketStream<TcpStream>>,
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
        rcon: Arc<Mutex<Option<Connection>>>,
        version_manager: Arc<Mutex<VersionManager>>,
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
            while let Ok(outgoing) = global_bus_rx.recv().await {
                let json = serde_json::to_string(&outgoing);
                match json {
                    Err(e) => {
                        error!("Error serialising message: {:?}", e)
                    }
                    Ok(json) => {
                        info!("Sending streaming message: {}", json);
                        AgentController::_send_message(
                            Arc::clone(&ws_tx_clone),
                            Message::Text(json),
                        )
                        .await;
                    }
                }
            }
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
            rcon,
            version_manager,
            global_tx: global_bus_tx,
            ws_rx,
            ws_tx,
            _send_global_outgoing_msgs_task,
            _sigint_task,
        })
    }

    async fn message_loop(mut self) -> tungstenite::Result<()> {
        while let Some(msg) = self.ws_rx.next().await {
            match msg {
                Ok(Message::Text(json)) => {
                    if let Ok(msg) = serde_json::from_str::<AgentRequestWithId>(&json) {
                        info!("Got incoming message from {}: {}", self.peer_addr, json);
                        let operation_id = msg.operation_id;
                        match msg.message {
                            AgentRequest::VersionInstall {
                                version,
                                force_install,
                            } => {
                                self.version_install(version, force_install, operation_id)
                                    .await
                            }
                            AgentRequest::ServerStart(savefile) => {
                                self.server_start(savefile, operation_id).await
                            }

                            AgentRequest::ServerStop => self.server_stop(operation_id).await,

                            AgentRequest::ServerStatus => self.server_status(operation_id).await,

                            AgentRequest::SaveCreate(save_name) => {
                                self.save_create(save_name, operation_id).await
                            }

                            AgentRequest::ConfigAdminListGet => {
                                self.config_admin_list_get(operation_id).await;
                            }

                            AgentRequest::ConfigAdminListSet { admins } => {
                                self.config_admin_list_set(admins, operation_id).await;
                            }

                            AgentRequest::ConfigRconGet => {
                                self.config_rcon_get(operation_id).await;
                            }

                            AgentRequest::ConfigRconSet { password } => {
                                self.config_rcon_set(password, operation_id).await;
                            }

                            AgentRequest::ConfigServerSettingsGet => {
                                self.config_server_settings_get(operation_id).await;
                            }

                            AgentRequest::ConfigServerSettingsSet { json } => {
                                self.config_server_settings_set(json, operation_id).await;
                            }

                            AgentRequest::RconCommand(cmd) => {
                                self.rcon_command(cmd, operation_id).await
                            }
                        }
                    }
                }
                Ok(Message::Binary(_)) => {
                    warn!("got binary message??");
                }
                Ok(Message::Ping(_)) => {
                    self.pong().await;
                }
                Ok(Message::Close(_)) => {
                    info!("Got close message from {}", self.peer_addr);
                    break;
                }
                Ok(_) => {
                    warn!("got other message");
                }
                Err(e) => {
                    error!("error with incoming message: {:?}", e);
                }
            }
        }

        info!("Cleaning up for peer {}", self.peer_addr);
        self._send_global_outgoing_msgs_task.abort();
        self._sigint_task.abort();
        Ok(())
    }

    async fn pong(&self) {
        if let Err(_e) = self
            .ws_tx
            .lock()
            .await
            .send(Message::Pong("Pong".to_owned().into_bytes()))
            .await
        {
            error!("Failed to send pong response to ping");
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

    async fn send_streaming_messsage(&self, message: AgentStreamingMessage) {
        let json = serde_json::to_string(&message);
        match json {
            Err(e) => {
                error!("Error serialising message: {:?}", e)
            }
            Ok(json) => {
                info!("Sending streaming message: {}", json);
                AgentController::_send_message(Arc::clone(&self.ws_tx), Message::Text(json)).await;
            }
        }
    }

    async fn reply(&self, message: AgentOutMessage, operation_id: &OperationId) {
        let with_id = AgentResponseWithId {
            operation_id: operation_id.clone(),
            status: OperationStatus::Ongoing,
            content: message,
        };
        let json = serde_json::to_string(&with_id);
        match json {
            Err(e) => {
                error!("Error serialising message: {:?}", e);
            }
            Ok(json) => {
                info!("Sending reply: {}", json);
                AgentController::_send_message(Arc::clone(&self.ws_tx), Message::Text(json)).await;
            }
        }
    }

    async fn reply_success(&self, message: AgentOutMessage, operation_id: OperationId) {
        let with_id = AgentResponseWithId {
            operation_id,
            status: OperationStatus::Completed,
            content: message,
        };
        let json = serde_json::to_string(&with_id);
        match json {
            Err(e) => {
                error!("Error serialising message: {:?}", e);
            }
            Ok(json) => {
                info!("Sending reply_success: {}", json);
                AgentController::_send_message(Arc::clone(&self.ws_tx), Message::Text(json)).await;
            }
        }
    }

    async fn reply_failed(&self, message: AgentOutMessage, operation_id: OperationId) {
        let with_id = AgentResponseWithId {
            operation_id,
            status: OperationStatus::Failed,
            content: message,
        };
        let json = serde_json::to_string(&with_id);
        match json {
            Err(e) => {
                error!("Error serialising message: {:?}", e);
            }
            Ok(json) => {
                info!("Sending reply_failed: {}", json);
                AgentController::_send_message(Arc::clone(&self.ws_tx), Message::Text(json)).await;
            }
        }
    }

    async fn version_install(
        &self,
        version_to_install: String,
        force_install: bool,
        operation_id: OperationId,
    ) {
        let mut vm = self.version_manager.lock().await;

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
                    return;
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
    }

    async fn server_start(&self, savefile: ServerStartSaveFile, operation_id: OperationId) {
        // assume there is at most one version installed
        let vm = self.version_manager.lock().await;
        let version;
        match vm.versions.values().next() {
            None => {
                self.reply_failed(
                    AgentOutMessage::Error("No installations of factorio detected".to_owned()),
                    operation_id,
                )
                .await;
                return;
            }
            Some(v) => {
                version = v;
            }
        }

        self.internal_server_start_with_version(version, savefile, operation_id, None)
            .await;
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
        // TODO mod folder
        let mods;
        match ModManager::read_or_apply_default().await {
            Ok(m) => mods = m,
            Err(_e) => {
                self.reply_failed(
                    AgentOutMessage::Error("Failedto read or initialise mod directory".to_owned()),
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

        let stream_out = Arc::clone(&self.global_tx);
        let mut builder = ServerBuilder::using_installation(version)
            .with_stdout_handler(move |s| {
                let msg = AgentStreamingMessage::ServerStdout(s);
                let _ = stream_out.send(msg);
            })
            .hosting_savefile(savefile, mods, admin_list, launch_settings, server_settings);

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
            server::proc::ProcessStatus::Running(s) => match s {
                server::InternalServerState::Ready
                | server::InternalServerState::PreparedToHostGame
                | server::InternalServerState::CreatingGame => ServerStatus::PreGame,
                server::InternalServerState::InGame => ServerStatus::InGame,
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
        let version_mg = self.version_manager.lock().await;
        let version;
        match version_mg.versions.values().next() {
            None => {
                self.reply_failed(
                    AgentOutMessage::Error("No installations of Factorio detected".to_owned()),
                    operation_id,
                )
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
                let msg = AgentStreamingMessage::ServerStdout(s);
                let _ = stream_out.send(msg);
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

    async fn config_rcon_get(&self, operation_id: OperationId) {
        match LaunchSettings::read_or_apply_default().await {
            Ok(ls) => {
                self.reply_success(
                    AgentOutMessage::ConfigRcon {
                        password: ls.rcon_password,
                        port: ls.rcon_bind.port(),
                    },
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

    async fn config_server_settings_get(&self, operation_id: OperationId) {
        if let Ok(Some(ss)) = ServerSettings::read().await {
            self.reply_success(AgentOutMessage::ConfigServerSettings(ss.json), operation_id)
                .await;
            return;
        }

        let vm = self.version_manager.lock().await;
        if let Some((_, version)) = vm.versions.iter().next() {
            match ServerSettings::read_or_apply_default(version).await {
                Ok(ss) => {
                    self.reply_success(
                        AgentOutMessage::ConfigServerSettings(ss.json),
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

    async fn config_server_settings_set(&self, json: String, operation_id: OperationId) {
        match ServerSettings::set(json).await {
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

    async fn rcon_command(&self, cmd: String, operation_id: OperationId) {
        // let mut mg = self.rcon.as_ref().lock().await;
        // if let Some(rcon) = mg.as_mut() {
        //     if let Err(e) = rcon.cmd(&cmd).await {
        //         error!("Couldn't send message to rcon: {:?}", e)
        //     }
        // }
        // TODO
        let _ = cmd;
        self.reply_failed(
            AgentOutMessage::Error("Not yet implemented".to_owned()),
            operation_id,
        )
        .await;
    }
}

// async fn rcon_connect(rcon_config: &RconConfig) -> Result<rcon::Connection, rcon::Error> {
//     info!("Attempting to connect to RCON at {}", rcon_config.address);
//     let conn = rcon::Connection::builder()
//         .enable_factorio_quirks(true)
//         .connect(rcon_config.address.to_owned(), &rcon_config.password)
//         .await?;
//     info!("Connected to RCON at {}", rcon_config.address);
//     Ok(conn)
// }

// fn try_parse_console_out_message(line: &str) -> Option<ConsoleOutMessage> {
//     lazy_static! {
//         static ref CHAT_RE: Regex =
//             Regex::new(r"^(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}) \[CHAT\] ([^:]+): (.+)$").unwrap();
//         static ref JOIN_RE: Regex = Regex::new(
//             r"^(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}) \[JOIN\] ([^:]+): joined the game$"
//         )
//         .unwrap();
//         static ref LEAVE_RE: Regex =
//             Regex::new(r"^(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}) \[LEAVE\] ([^:]+) left the game$")
//                 .unwrap();
//     }

//     if let Some(chat_captures) = CHAT_RE.captures(line) {
//         let timestamp = chat_captures.get(1).unwrap().as_str().to_string();
//         let user = chat_captures.get(2).unwrap().as_str().to_string();
//         let msg = chat_captures.get(3).unwrap().as_str().to_string();
//         Some(ConsoleOutMessage::Chat {
//             timestamp,
//             user,
//             msg,
//         })
//     } else if let Some(join_captures) = JOIN_RE.captures(line) {
//         let timestamp = join_captures.get(1).unwrap().as_str().to_string();
//         let user = join_captures.get(2).unwrap().as_str().to_string();
//         Some(ConsoleOutMessage::Join { timestamp, user })
//     } else if let Some(leave_captures) = LEAVE_RE.captures(line) {
//         let timestamp = leave_captures.get(1).unwrap().as_str().to_string();
//         let user = leave_captures.get(2).unwrap().as_str().to_string();
//         Some(ConsoleOutMessage::Leave { timestamp, user })
//     } else {
//         None
//     }
// }
