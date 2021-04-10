use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
};

use crate::consts::*;
use crate::schema::*;
use crate::server::builder::ServerBuilder;
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use lazy_static::lazy_static;
use log::{debug, error, info, warn};
use logwatcher::LogWatcher;
use rcon::Connection;
use regex::Regex;
use server::proc::ProcessManager;
use tokio::{
    fs,
    net::{TcpListener, TcpStream},
    sync::{broadcast, Mutex},
    task::JoinHandle,
};
use tokio_tungstenite::{accept_async, tungstenite, WebSocketStream};
use tungstenite::Message;

mod consts;
mod error;
mod factorio;
mod schema;
mod server;
mod util;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    info!("Reading config");
    let config_str = fs::read_to_string("config.toml")
        .await
        .expect("Couldn't read config.toml");
    let config: Config = toml::from_str(&config_str)?;

    info!("Init Factorio installation manager");
    let version_manager = factorio::VersionManager::new(FACTORIO_INSTALL_DIR).await?;

    info!("Init Factorio server process management");
    let proc_manager = ProcessManager::new();

    info!("Init RCON");
    let rcon;
    if let Some(rcon_config) = config.rcon {
        rcon = Some(rcon_connect(&rcon_config).await?);
    } else {
        warn!("No RCON connection established as config section is missing");
        rcon = None;
    }

    let (global_bus_tx, ..) = broadcast::channel::<OutgoingMessage>(50);

    info!("Init console out features");
    if let Some(console_out_config) = config.console {
        let console_out_events_tx = global_bus_tx.clone();
        tokio::task::spawn_blocking(move || {
            info!(
                "Watching console out file {}",
                console_out_config.console_log_path
            );
            let mut watcher = LogWatcher::register(console_out_config.console_log_path)
                .expect("Could not register watcher for console out");
            watcher.watch(&mut move |line| {
                if let Some(msg) = try_parse_console_out_message(&line) {
                    if let Err(e) = console_out_events_tx.send(OutgoingMessage::ConsoleOut(msg)) {
                        error!("Could not enqueue console out event: {:?}", e);
                    }
                }

                logwatcher::LogWatcherAction::None
            });
        });
    } else {
        warn!("No console out features as config section is missing");
    }

    info!("Init WebSocketListener");
    let ws_listener = WebSocketListener::new(&config.agent).await?;

    info!("Listening on {}", ws_listener.tcp.local_addr()?);
    ws_listener
        .run(
            Arc::new(global_bus_tx),
            Arc::new(proc_manager),
            Arc::new(Mutex::new(rcon)),
            Arc::new(Mutex::new(version_manager)),
        )
        .await;

    Ok(())
}

struct WebSocketListener {
    tcp: TcpListener,
}

impl WebSocketListener {
    async fn new(config: &AgentConfig) -> Result<WebSocketListener, std::io::Error> {
        let tcp = TcpListener::bind(&config.websocket_bind_address).await?;
        Ok(WebSocketListener { tcp })
    }

    async fn run(
        self,
        global_bus_tx: Arc<broadcast::Sender<OutgoingMessage>>,
        proc_manager: Arc<ProcessManager>,
        rcon: Arc<Mutex<Option<Connection>>>,
        version_manager: Arc<Mutex<factorio::VersionManager>>,
    ) {
        while let Ok((stream, _)) = self.tcp.accept().await {
            match AgentController::handle_connection(
                stream,
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
}

struct AgentController {
    peer_addr: SocketAddr,
    proc_manager: Arc<ProcessManager>,
    rcon: Arc<Mutex<Option<Connection>>>,
    version_manager: Arc<Mutex<factorio::VersionManager>>,
    ws_rx: SplitStream<WebSocketStream<TcpStream>>,
    ws_tx: Arc<Mutex<SplitSink<WebSocketStream<TcpStream>, Message>>>,
    _send_global_outgoing_msgs_task: JoinHandle<()>,
}

impl AgentController {
    async fn handle_connection(
        tcp: TcpStream,
        global_bus_tx: Arc<broadcast::Sender<OutgoingMessage>>,
        proc_manager: Arc<ProcessManager>,
        rcon: Arc<Mutex<Option<Connection>>>,
        version_manager: Arc<Mutex<factorio::VersionManager>>,
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
                match serde_json::to_string(&outgoing) {
                    Ok(json) => {
                        if let Err(e) = ws_tx_clone.lock().await.send(Message::Text(json)).await {
                            error!("Could not send outgoing message: {:?}", e);
                        }
                    }
                    Err(e) => {
                        error!("Could not serialise outgoing message to json: {:?}", e);
                    }
                }
            }
        });

        Ok(AgentController {
            peer_addr,
            proc_manager,
            rcon,
            version_manager,
            ws_rx,
            ws_tx,
            _send_global_outgoing_msgs_task,
        })
    }

    async fn message_loop(mut self) -> tungstenite::Result<()> {
        debug!("In message loop");
        while let Some(msg) = self.ws_rx.next().await {
            debug!("Got message");
            match msg {
                Ok(Message::Text(json)) => {
                    debug!("Is a text message, contents: {}", json);
                    if let Ok(msg) = serde_json::from_str::<IncomingMessage>(&json) {
                        info!("Got incoming message from {}: {}", self.peer_addr, json);
                        match msg {
                            IncomingMessage::InitWithVersion(version) => {
                                debug!("handling: InitWithVersion({})", version);
                                self.init_with_version(version).await
                            }
                            IncomingMessage::UpgradeVersion(version) => {
                                debug!("handling: UpgradeVersion({})", version);
                                self.upgrade_version(version).await
                            }
                            IncomingMessage::ServerStart(savefile) => {
                                debug!("handling: ServerStart({:?})", savefile);
                                self.server_start(savefile).await
                            }

                            IncomingMessage::ServerStop => {
                                debug!("handling: ServerStop");
                                self.server_stop().await
                            }

                            IncomingMessage::SaveCreate(save_name) => {
                                debug!("handling: CreateSave({})", save_name);
                                self.save_create(save_name).await;
                            }

                            IncomingMessage::ServerStatus => todo!(),

                            IncomingMessage::ChatPrint(chat_msg) => self.chat_print(chat_msg).await,

                            IncomingMessage::RconCommand(cmd) => self.rcon_command(cmd).await,
                        }
                    }
                }
                Ok(Message::Binary(_)) => {
                    warn!("got binary message??");
                }
                Ok(Message::Ping(_)) => {
                    self.send_message(Message::Pong("Pong".to_owned().into_bytes()))
                        .await;
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
        Ok(())
    }

    async fn send_message(&self, message: Message) {
        debug!("sending message: {}", message);
        if let Err(e) = self.ws_tx.lock().await.send(message).await {
            error!("Error sending message: {:?}", e);
        }
    }

    async fn init_with_version(&self, version: String) {
        let mut vm = self.version_manager.lock().await;

        // Should only be run if no currently installed versions
        if vm.versions.is_empty() {
            if let Err(e) = vm.install(version).await {
                self.send_message(Message::Text(format!("Error: failed to install: {:?}", e)))
                    .await;
            } else {
                self.send_message(Message::Text("Ok".to_owned())).await;
            }
        } else {
            self.send_message(Message::Text(
                "Error: cannot init as there is already a currently installed version".to_owned(),
            ))
            .await;
        }
    }

    async fn upgrade_version(&self, version: String) {
        let mut vm = self.version_manager.lock().await;

        if vm.versions.is_empty() {
            self.send_message(Message::Text("Error: must run init first".to_owned()))
                .await;
        } else {
            // Assume there is at most one version installed
            let current_version_string = vm.versions.keys().next().unwrap().to_string();

            // Install specified version
            info!("Installing version {} for upgrade", version);
            if let Err(e) = vm.install(version.clone()).await {
                self.send_message(Message::Text(format!("Error: failed to install: {:?}", e)))
                    .await;
                return;
            }

            // Stop server if running
            info!("Stopping server for upgrade");
            let opt_stopped_instance = self.proc_manager.stop_instance().await;
            let opt_previous_savefile = opt_stopped_instance.map(|si| si.savefile).flatten();

            // TODO stage save migrations?

            // Remove previous version
            info!(
                "Removing previous version {} after upgrade",
                current_version_string
            );
            if let Err(e) = vm.delete(&current_version_string).await {
                self.send_message(Message::Text(format!("Error: failed to remove previous version {} after upgrading to version {}: {:?}", current_version_string, version, e))).await;
            }

            // Start server on new version if it was previously running
            if let Some(previous_savefile) = opt_previous_savefile {
                info!("Restarting server after upgrade");
                let new_version = vm.versions.get(&version).expect("just added several lines back and we still hold the lock, this should never fail");
                let builder = ServerBuilder::using_installation(new_version)
                    .with_savefile(previous_savefile)
                    .bind_on(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8080));

                if let Err(e) = self.proc_manager.run_instance(builder).await {
                    self.send_message(Message::Text(format!("Error: failed to start: {:?}", e)))
                        .await;
                } else {
                    self.send_message(Message::Text("Ok".to_owned())).await;
                }
            } else {
                self.send_message(Message::Text("Ok".to_owned())).await;
            }
        }
    }

    async fn server_start(&self, savefile: ServerStartSaveFile) {
        // assume there is at most one version installed
        let version_mg = self.version_manager.lock().await;
        let version;
        match version_mg.versions.values().next() {
            None => {
                self.send_message(Message::Text(
                    "Error: no installations of factorio detected".to_owned(),
                ))
                .await;
                return;
            }
            Some(v) => {
                version = v;
            }
        }

        let builder = ServerBuilder::using_installation(version)
            .with_savefile(savefile)
            .bind_on(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8080));

        if let Err(e) = self.proc_manager.run_instance(builder).await {
            self.send_message(Message::Text(format!("Error: failed to start: {:?}", e)))
                .await;
        }
    }

    async fn server_stop(&self) {
        self.proc_manager.stop_instance().await;
    }

    async fn server_status(&self) {
        todo!()
    }

    async fn save_create(&self, save_name: String) {
        // assume there is at most one version installed
        let version_mg = self.version_manager.lock().await;
        let version;
        match version_mg.versions.values().next() {
            None => {
                self.send_message(Message::Text(
                    "Error: no installations of factorio detected".to_owned(),
                ))
                .await;
                return;
            }
            Some(v) => {
                version = v;
            }
        }

        // Create save dir if not exists
        let save_dir = util::saves::get_save_dir_path();
        if let Err(e) = fs::create_dir_all(&save_dir).await {
            error!(
                "Failed to create save dir at {}: {:?}",
                save_dir.display(),
                e
            );
            self.send_message(Message::Text(format!(
                "Error: failed to create save dir: {:?}",
                e
            )))
            .await;
            return;
        }

        let builder = ServerBuilder::using_installation(version).creating_savefile(&save_name);
        if let Err(e) = self.proc_manager.run_instance(builder).await {
            self.send_message(Message::Text(format!("Error: failed to start: {:?}", e)))
                .await;
        }

        self.proc_manager.wait_for_instance().await;

        self.send_message(Message::Text(format!(
            "Successfully created savefile with name {}",
            save_name
        )))
        .await;
    }

    async fn chat_print(&self, msg: String) {
        self.rcon_command(format!("/silent-command game.print('{}')", msg))
            .await;
    }

    async fn rcon_command(&self, cmd: String) {
        let mut mg = self.rcon.as_ref().lock().await;
        if let Some(rcon) = mg.as_mut() {
            if let Err(e) = rcon.cmd(&cmd).await {
                error!("Couldn't send message to rcon: {:?}", e)
            }
        }
    }
}

async fn rcon_connect(rcon_config: &RconConfig) -> Result<rcon::Connection, rcon::Error> {
    info!("Attempting to connect to RCON at {}", rcon_config.address);
    let conn = rcon::Connection::builder()
        .enable_factorio_quirks(true)
        .connect(rcon_config.address.to_owned(), &rcon_config.password)
        .await?;
    info!("Connected to RCON at {}", rcon_config.address);
    Ok(conn)
}

fn try_parse_console_out_message(line: &str) -> Option<ConsoleOutMessage> {
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

    if let Some(chat_captures) = CHAT_RE.captures(line) {
        let timestamp = chat_captures.get(1).unwrap().as_str().to_string();
        let user = chat_captures.get(2).unwrap().as_str().to_string();
        let msg = chat_captures.get(3).unwrap().as_str().to_string();
        Some(ConsoleOutMessage::Chat {
            timestamp,
            user,
            msg,
        })
    } else if let Some(join_captures) = JOIN_RE.captures(line) {
        let timestamp = join_captures.get(1).unwrap().as_str().to_string();
        let user = join_captures.get(2).unwrap().as_str().to_string();
        Some(ConsoleOutMessage::Join { timestamp, user })
    } else if let Some(leave_captures) = LEAVE_RE.captures(line) {
        let timestamp = leave_captures.get(1).unwrap().as_str().to_string();
        let user = leave_captures.get(2).unwrap().as_str().to_string();
        Some(ConsoleOutMessage::Leave { timestamp, user })
    } else {
        None
    }
}
