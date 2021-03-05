#[macro_use]
extern crate log;
extern crate lazy_static;

use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use lazy_static::lazy_static;
use logwatcher::LogWatcher;
use regex::Regex;
use rcon::Connection;
use schema::*;
use tokio::{fs, net::{TcpListener, TcpStream}, sync::{Mutex, broadcast}};
use tokio_tungstenite::{accept_async, tungstenite};
use tungstenite::Message;

mod schema;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    info!("Reading config");
    let config_str = fs::read_to_string("config.toml")
        .await
        .expect("Couldn't read config.toml");
    let config: Config = toml::from_str(&config_str)?;

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
            info!("Watching console out file {}", console_out_config.console_log_path);
            let mut watcher = LogWatcher::register(console_out_config.console_log_path).expect("Could not register watcher for console out");
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

    info!("Init agent controller");
    let agent_controller = AgentController::new(config.agent, rcon, outgoing_tx.clone()).await?;

    info!("Listening on {}", agent_controller.tcp.local_addr()?);
    agent_controller.run().await;

    Ok(())
}

struct AgentController {
    rcon: Arc<Mutex<Option<Connection>>>,
    tcp: TcpListener,
    outgoing_events: Arc<broadcast::Sender<OutgoingMessage>>,
}

impl AgentController {
    async fn new(
        config: AgentConfig,
        rcon: Option<Connection>,
        outgoing_events: broadcast::Sender<OutgoingMessage>
    ) -> Result<AgentController, std::io::Error> {
        let tcp = TcpListener::bind(&config.bind_address).await?;
        Ok(AgentController {
            rcon: Arc::new(Mutex::new(rcon)),
            tcp,
            outgoing_events: Arc::new(outgoing_events),
        })
    }

    async fn run(self) {
        while let Ok((stream, _)) = self.tcp.accept().await {
            tokio::spawn(AgentController::accept_connection(
                stream,
                Arc::clone(&self.rcon),
                Arc::clone(&self.outgoing_events),
            ));
        }
    }

    async fn accept_connection(stream: TcpStream, rcon: Arc<Mutex<Option<Connection>>>, outgoing_events: Arc<broadcast::Sender<OutgoingMessage>>) {
        if let Err(e) = AgentController::handle_connection(stream, rcon, outgoing_events).await {
            match e {
                tungstenite::Error::ConnectionClosed
                | tungstenite::Error::Protocol(_)
                | tungstenite::Error::Utf8 => (),
                err => error!("Error handling connection: {}", err),
            }
        }
    }

    async fn handle_connection(
        stream: TcpStream,
        rcon: Arc<Mutex<Option<Connection>>>,
        outgoing_events: Arc<broadcast::Sender<OutgoingMessage>>,
    ) -> tungstenite::Result<()> {
        let peer_addr = stream.peer_addr()?;
        let ws = accept_async(stream).await?;
        let (ws_tx, mut ws_rx) = ws.split();
        let ws_tx = Arc::new(Mutex::new(ws_tx));
        info!("Peer connected: {}", peer_addr);

        let ws_tx_clone = Arc::clone(&ws_tx);
        let send_outgoing_events_task = tokio::spawn(async move {
            let mut rx = outgoing_events.subscribe();
            while let Ok(outgoing) = rx.recv().await {
                match serde_json::to_string(&outgoing) {
                    Ok(json) => {
                        if let Err(e) = ws_tx_clone.lock().await.send(Message::Text(json)).await {
                            error!("Could not send outgoing message: {:?}", e);
                        }
                    },
                    Err(e) => {
                        error!("Could not serialise outgoing message to json: {:?}", e);
                    },
                }
            }
        });

        while let Some(msg) = ws_rx.next().await {
            match msg? {
                Message::Text(json) => {
                    if let Ok(msg) = serde_json::from_str::<IncomingMessage>(&json) {
                        info!("Got incoming message: {}", json);
                        match msg {
                            IncomingMessage::ChatPrint(chat_msg) => {
                                AgentController::chat_print(Arc::clone(&rcon), chat_msg).await;
                            }
                            IncomingMessage::RconCommand(cmd) => {
                                AgentController::rcon_command(Arc::clone(&rcon), cmd).await;
                            }
                        }
                    }
                }
                // Message::Binary(_) => {}
                Message::Ping(_) => {
                    ws_tx.lock().await.send(Message::Pong("Pong".to_owned().into_bytes()))
                        .await?
                }
                Message::Close(_) => {
                    info!("Got close message from {}", peer_addr);
                    break;
                }
                _ => (),
            }
        }

        info!("Cleaning up for peer {}", peer_addr);
        send_outgoing_events_task.abort();
        Ok(())
    }

    async fn chat_print(rcon: Arc<Mutex<Option<Connection>>>, msg: String) {
        AgentController::rcon_command(rcon, format!("/silent-command game.print('{}')", msg)).await;
    }

    async fn rcon_command(rcon: Arc<Mutex<Option<Connection>>>, cmd: String) {
        let mut mg = rcon.as_ref().lock().await;
        if let Some(rcon) = mg.as_mut() {
            if let Err(e) = rcon.cmd(&format!("{}", cmd)).await {
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
        static ref CHAT_RE: Regex = Regex::new(r"^(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}) \[CHAT\] ([^:]+): (.+)$").unwrap();
        static ref JOIN_RE: Regex = Regex::new(r"^(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}) \[JOIN\] ([^:]+): joined the game$").unwrap();
        static ref LEAVE_RE: Regex = Regex::new(r"^(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}) \[LEAVE\] ([^:]+) left the game$").unwrap();
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
        Some(ConsoleOutMessage::Join {
            timestamp,
            user,
        })
    } else if let Some(leave_captures) = LEAVE_RE.captures(line) {
        let timestamp = leave_captures.get(1).unwrap().as_str().to_string();
        let user = leave_captures.get(2).unwrap().as_str().to_string();
        Some(ConsoleOutMessage::Leave {
            timestamp,
            user,
        })
    } else {
        None
    }
}