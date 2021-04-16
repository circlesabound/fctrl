use fctrl::schema::*;

use futures::{Sink, Stream};
use futures_util::sink::SinkExt;
use futures_util::StreamExt;
use lazy_static::lazy_static;
use std::io::Write;
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::{self, Message};

lazy_static! {
    /// Pipe mode - disable output decoratives to facilitate piping input/output
    static ref PIPE_MODE: Mutex<bool> = Mutex::new(false);
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async {
            let addr_str = std::env::args()
                .nth(1)
                .expect("expecting arg for websocket address");
            let addr = url::Url::parse(addr_str.trim())?;

            if let Some(s) = std::env::args().nth(2) {
                if s == "--pipe-mode" {
                    *(PIPE_MODE.lock().await) = true;
                }
            }

            let (ws_stream, ..) = tokio_tungstenite::connect_async(addr).await?;
            let (ws_write, ws_read) = ws_stream.split();
            if !is_pipe_mode().await {
                println!("Connected");
            }

            message_loop(ws_write, ws_read).await?;

            Ok(())
        })
}

async fn is_pipe_mode() -> bool {
    PIPE_MODE.lock().await.clone()
}

async fn message_loop<W, R>(
    mut ws_write: W,
    mut ws_read: R,
) -> Result<(), Box<dyn std::error::Error>>
where
    W: Sink<Message> + Unpin,
    R: Stream<Item = Result<Message, tungstenite::Error>> + Unpin,
{
    loop {
        if !is_pipe_mode().await {
            print!("> ");
            std::io::stdout().flush()?;
        }
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if input.trim().is_empty() {
            break;
        }

        match get_message_from_input(input) {
            None => {
                println!("?")
            }
            Some(req) => {
                if ws_write
                    .send(Message::Text(serde_json::to_string(&req).unwrap()))
                    .await
                    .is_err()
                {
                    println!("Error sending message");
                    continue;
                }

                // wait for replies
                loop {
                    let reply = ws_read.next().await.unwrap().unwrap();
                    if let Message::Text(json) = reply {
                        let reply: AgentResponseWithId = serde_json::from_str(&json).unwrap();
                        println!("{}", json);
                        match reply.status {
                            OperationStatus::Completed | OperationStatus::Failed => {
                                break;
                            }
                            OperationStatus::Ongoing => {
                                // more messages on the way
                            }
                        }
                    } else {
                        println!("received unknown reply");
                        break;
                    }
                }
            }
        }
    }

    if ws_write.close().await.is_err() {
        println!("Error closing connection cleanly");
    }

    Ok(())
}

fn get_message_from_input(input: String) -> Option<AgentRequestWithId> {
    let operation_id = OperationId::from(uuid::Uuid::new_v4().to_string());
    let args: Vec<_> = input.trim().split_whitespace().collect();
    match args.get(0)? {
        &"VersionInstall" => args.get(1).map(|v| AgentRequestWithId {
            operation_id,
            message: AgentRequest::VersionInstall(v.to_string()),
        }),
        &"ServerStart" => args
            .get(1)
            .map(|savefile| {
                if *savefile == "Latest" {
                    Some(AgentRequestWithId {
                        operation_id,
                        message: AgentRequest::ServerStart(ServerStartSaveFile::Latest),
                    })
                } else if *savefile == "Specific" {
                    args.get(2).map(|name| AgentRequestWithId {
                        operation_id,
                        message: AgentRequest::ServerStart(ServerStartSaveFile::Specific(
                            name.to_string(),
                        )),
                    })
                } else {
                    None
                }
            })
            .flatten(),
        &"ServerStop" => Some(AgentRequestWithId {
            operation_id,
            message: AgentRequest::ServerStop,
        }),
        &"ServerStatus" => Some(AgentRequestWithId {
            operation_id,
            message: AgentRequest::ServerStatus,
        }),
        &"SaveCreate" => args.get(1).map(|name| AgentRequestWithId {
            operation_id,
            message: AgentRequest::SaveCreate(name.to_string()),
        }),
        &"RconCommand" => args.get(1).map(|cmd| AgentRequestWithId {
            operation_id,
            message: AgentRequest::RconCommand(cmd.to_string()),
        }),
        _ => None,
    }
}
