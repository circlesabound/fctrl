#[macro_use]
extern crate log;
extern crate lazy_static;

use std::{net::SocketAddr, sync::Arc};

use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use lazy_static::lazy_static;
use logwatcher::LogWatcher;
use rcon::Connection;
use regex::Regex;
use schema::*;
use tokio::{
    fs,
    net::{TcpListener, TcpStream},
    sync::{broadcast, Mutex},
    task::JoinHandle,
};
use tokio_tungstenite::{accept_async, tungstenite, WebSocketStream};
use tungstenite::Message;

mod error;
mod factorio;
mod schema;
mod server;
mod util;

const FACTORIO_INSTALL_DIR: &str = "install";

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

    info!("Init RCON");
    let rcon;
    if let Some(rcon_config) = config.rcon {
        rcon = Some(rcon_connect(&rcon_config).await?);
    } else {
        warn!("No RCON connection established as config section is missing");
        rcon = None;
    }

    let (outgoing_tx, ..) = broadcast::channel::<OutgoingMessage>(10);

    info!("Init console out features");
    if let Some(console_out_config) = config.console {
        let console_out_events_tx = outgoing_tx.clone();
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
            Arc::new(outgoing_tx),
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
        let tcp = TcpListener::bind(&config.bind_address).await?;
        Ok(WebSocketListener { tcp })
    }

    async fn run(
        self,
        outgoing_events: Arc<broadcast::Sender<OutgoingMessage>>,
        rcon: Arc<Mutex<Option<Connection>>>,
        version_manager: Arc<Mutex<factorio::VersionManager>>,
    ) {
        while let Ok((stream, _)) = self.tcp.accept().await {
            match AgentController::handle_connection(
                stream,
                Arc::clone(&outgoing_events),
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
    rcon: Arc<Mutex<Option<Connection>>>,
    version_manager: Arc<Mutex<factorio::VersionManager>>,
    ws_rx: SplitStream<WebSocketStream<TcpStream>>,
    ws_tx: Arc<Mutex<SplitSink<WebSocketStream<TcpStream>, Message>>>,
    _send_outgoing_events_task: JoinHandle<()>,
}

impl AgentController {
    async fn handle_connection(
        tcp: TcpStream,
        outgoing_events: Arc<broadcast::Sender<OutgoingMessage>>,
        rcon: Arc<Mutex<Option<Connection>>>,
        version_manager: Arc<Mutex<factorio::VersionManager>>,
    ) -> tungstenite::Result<AgentController> {
        let peer_addr = tcp.peer_addr()?;
        let ws = accept_async(tcp).await?;
        let (ws_tx, ws_rx) = ws.split();
        let ws_tx = Arc::new(Mutex::new(ws_tx));
        info!("WebSocket peer connected: {}", peer_addr);

        // Set up background task to deliver outgoing messages from the broadcast queue
        let ws_tx_clone = Arc::clone(&ws_tx);
        let mut outgoing_events_rx = outgoing_events.subscribe();
        let _send_outgoing_events_task = tokio::spawn(async move {
            while let Ok(outgoing) = outgoing_events_rx.recv().await {
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
            rcon,
            version_manager,
            ws_rx,
            ws_tx,
            _send_outgoing_events_task,
        })
    }

    async fn message_loop(mut self) -> tungstenite::Result<()> {
        while let Some(msg) = self.ws_rx.next().await {
            match msg? {
                Message::Text(json) => {
                    if let Ok(msg) = serde_json::from_str::<IncomingMessage>(&json) {
                        info!("Got incoming message from {}: {}", self.peer_addr, json);
                        match msg {
                            IncomingMessage::InitWithVersion(version) => {
                                self.init_with_version(version).await
                            }
                            IncomingMessage::UpgradeVersion(version) => {
                                self.upgrade_version(version).await
                            }

                            IncomingMessage::ServerStart => todo!(),

                            IncomingMessage::ServerStop => todo!(),

                            IncomingMessage::ServerStatus => todo!(),

                            IncomingMessage::ChatPrint(chat_msg) => self.chat_print(chat_msg).await,

                            IncomingMessage::RconCommand(cmd) => self.rcon_command(cmd).await,
                        }
                    }
                }
                // Message::Binary(_) => {}
                Message::Ping(_) => {
                    self.send_message(Message::Pong("Pong".to_owned().into_bytes()))
                        .await;
                }
                Message::Close(_) => {
                    info!("Got close message from {}", self.peer_addr);
                    break;
                }
                _ => (),
            }
        }

        info!("Cleaning up for peer {}", self.peer_addr);
        self._send_outgoing_events_task.abort();
        Ok(())
    }

    async fn send_message(&self, message: Message) {
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
            // Install specified version
            if let Err(e) = vm.install(version).await {
                self.send_message(Message::Text(format!("Error: failed to install: {:?}", e)))
                    .await;
                return;
            }

            // TODO save migrations
            // TODO mark new current version
            // TODO Remove previous version?
        }
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
