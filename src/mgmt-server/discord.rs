use std::sync::Arc;

use futures::{StreamExt, pin_mut};
use log::{error, info};
use serenity::{client::{Context, EventHandler}, model::{channel::Message, prelude::*}};
use tokio::{sync::mpsc, task::JoinHandle};

use crate::{clients::AgentApiClient, error::Result, events::{CHAT_TOPIC_NAME, JOIN_TOPIC_NAME, LEAVE_TOPIC_NAME, TopicName, broker::EventBroker}};

pub struct DiscordClient {
    _jh: JoinHandle<()>,
}

impl DiscordClient {
    pub async fn new(
        bot_token: String,
        chat_link_channel_id: Option<u64>,
        agent_client: Arc<AgentApiClient>,
        event_broker: Arc<EventBroker>,
    ) -> Result<DiscordClient> {
        let mut client_builder = serenity::Client::builder(&bot_token);
        if let Some(chat_link_channel_id) = chat_link_channel_id {
            let d2g = DiscordToGameChatLinkHandler {
                agent_client,
                listen_channel_id: chat_link_channel_id,
            };
            client_builder = client_builder.event_handler(d2g);
        } else {
            info!("Discord chat link channel id not provided, chat link functionality will be disabled");
        }
        let mut client = client_builder.await?;

        let jh = tokio::spawn(async move {
            if let Err(e) = client.start().await {
                error!("Error with Discord client: {:?}", e);
            }
        });

        if let Some(chat_link_channel_id) = chat_link_channel_id {
            DiscordClient::create_chat_link_g2d_subscriber(bot_token, chat_link_channel_id, event_broker).await?;
        }

        Ok(DiscordClient {
            _jh: jh,
        })
    }

    async fn create_chat_link_g2d_subscriber(
        bot_token: String,
        chat_link_channel_id: u64,
        event_broker: Arc<EventBroker>,
    ) -> crate::error::Result<()> {
        let (chat_tx, mut rx) = mpsc::unbounded_channel();
        let join_tx = chat_tx.clone();
        let leave_tx = chat_tx.clone();

        tokio::spawn(async move {
            let http = serenity::http::Http::new_with_token(&bot_token);
            let channel = ChannelId(chat_link_channel_id);
            while let Some(line) = rx.recv().await {
                if let Err(e) = channel.say(&http, line).await {
                    error!("Couldn't send message to Discord: {:?}", e);
                }
            }
        });

        let chat_sub = event_broker
            .subscribe(TopicName(CHAT_TOPIC_NAME.to_string()), |_| true)
            .await;
        tokio::spawn(async move {
            pin_mut!(chat_sub);
            while let Some(event) = chat_sub.next().await {
                let message = event.tags.get(&TopicName(CHAT_TOPIC_NAME.to_string())).unwrap();
                if let Err(e) = chat_tx.send(message.clone()) {
                    error!("Error sending line through mpsc channel: {:?}", e);
                    break;
                }
            }

            error!("Discord chat link g2d chat subscriber is finishing, this should never happen!");
        });

        let join_sub = event_broker
            .subscribe(TopicName(JOIN_TOPIC_NAME.to_string()), |_| true)
            .await;
        tokio::spawn(async move {
            pin_mut!(join_sub);
            while let Some(event) = join_sub.next().await {
                let user = event.tags.get(&TopicName(JOIN_TOPIC_NAME.to_string())).unwrap();
                let message = format!("**{} has joined the server**", user);
                if let Err(e) = join_tx.send(message) {
                    error!("Error sending line through mpsc channel: {:?}", e);
                    break;
                }
            }

            error!("Discord chat link g2d join subscriber is finishing, this should never happen!");
        });

        let leave_sub = event_broker
            .subscribe(TopicName(LEAVE_TOPIC_NAME.to_string()), |_| true)
            .await;
        tokio::spawn(async move {
            pin_mut!(leave_sub);
            while let Some(event) = leave_sub.next().await {
                let user = event.tags.get(&TopicName(LEAVE_TOPIC_NAME.to_string())).unwrap();
                let message = format!("**{} has left the server**", user);
                if let Err(e) = leave_tx.send(message) {
                    error!("Error sending line through mpsc channel: {:?}", e);
                    break;
                }
            }

            error!("Discord chat link g2d leave subscriber is finishing, this should never happen!");
        });

        Ok(())
    }
}

struct DiscordToGameChatLinkHandler {
    agent_client: Arc<AgentApiClient>,
    listen_channel_id: u64,
}

#[serenity::async_trait]
impl EventHandler for DiscordToGameChatLinkHandler {
    async fn message(&self, _ctx: Context, msg: Message) {
        if msg.channel_id == self.listen_channel_id && !msg.author.bot {
            // TODO handle empty messages with embeds, attachments, etc
            let message_text = format!("{}: {}", msg.author.name, msg.content);
            let message_text = message_text.replace('\\', "\\\\");
            let message_text = message_text.replace('\'', "\\'");
            let command = format!("/silent-command game.print('[Discord] {}')", message_text);
            if let Err(e) = self.agent_client.rcon_command(command).await {
                error!("Couldn't send message via agent_client rcon_command: {:?}", e);
            }
        }
    }

    async fn ready(&self, _ctx: Context, _ready: Ready) {
        info!("DiscordToGameChatLinkHandler ready");
    }
}
