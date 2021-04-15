

use futures::{Sink, Stream};
use futures_util::StreamExt;
use futures_util::sink::SinkExt;
use tokio_tungstenite::tungstenite::{self, Message};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            print!("Connect to websocket address: ");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            let addr = url::Url::parse(input.trim())?;

            let (ws_stream, ..) = tokio_tungstenite::connect_async(addr).await?;
            let (ws_write, ws_read) = ws_stream.split();
            println!("Connected");

            message_loop(ws_write, ws_read).await?;

            Ok(())
        })
}

async fn message_loop<W, R>(mut ws_write: W, mut ws_read: R) -> Result<(), Box<dyn std::error::Error>>
where
    W: Sink<Message> + Unpin,
    R: Stream<Item = Result<Message, tungstenite::Error>> + Unpin,
{
    loop {
        print!("> ");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if input.trim().is_empty() {
            break;
        }

        match get_message_from_input(input) {
            None => {
                println!("?")
            },
            Some(msg) => {
                ws_write.send(msg).await;

                // wait for replies
                loop {
                    let reply = ws_read.next().await.unwrap().unwrap();
                    if let Message::Text(json) = reply {
                        let reply: OutgoingMessageWithId = serde_json::from_str(&json).unwrap();
                    } else {
                        println!("received unknown reply");
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}

fn get_message_from_input(input: String) -> Option<Message> {
    let args: Vec<_> = input.trim().split_whitespace().collect();
    match args[1] {
        "VersionInstall" => {
            todo!()
        },
        "ServerStart" => {
            if args[2] == "Latest" {
                //
            } else {

            }
        }
        _ => println!("?"),
    };
}
