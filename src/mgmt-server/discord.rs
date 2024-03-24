use std::sync::Arc;
use std::{collections::HashMap, time::Duration};

use fctrl::schema::ServerStatus;
use futures::{pin_mut, StreamExt};
use log::{error, info, warn};
use serenity::gateway::ActivityData;
use serenity::{
    client::{Cache, Context, EventHandler},
    http::Http,
    model::prelude::*,
    utils::MessageBuilder,
};
use tokio::{sync::mpsc, task::JoinHandle};

use crate::{
    clients::AgentApiClient,
    error::{Error, Result},
    events::{broker::EventBroker, TopicName, CHAT_TOPIC_NAME, JOIN_TOPIC_NAME, LEAVE_TOPIC_NAME},
};

pub struct DiscordClient {
    alert_tx: Option<mpsc::UnboundedSender<String>>,
    alert_channel_http: Option<Http>,
    alert_channel_id: Option<u64>,
    cache: Arc<Cache>,
    _jh: JoinHandle<()>,
}

impl DiscordClient {
    pub async fn new(
        bot_token: String,
        alert_channel_id: Option<u64>,
        chat_link_channel_id: Option<u64>,
        agent_client: Arc<AgentApiClient>,
        event_broker: Arc<EventBroker>,
    ) -> Result<DiscordClient> {
        let cache = Arc::new(Cache::new());
        let gateway_intents = GatewayIntents::default() | GatewayIntents::MESSAGE_CONTENT;
        let mut client_builder = serenity::Client::builder(&bot_token, gateway_intents);
        if let Some(chat_link_channel_id) = chat_link_channel_id {
            let handler = Handler {
                agent_client: Arc::clone(&agent_client),
                listen_channel_id: chat_link_channel_id,
            };
            client_builder = client_builder.event_handler(handler);
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
            let bot_token_clone = bot_token.clone();
            let (chat_link_tx, mut rx) = mpsc::unbounded_channel();
            tokio::spawn(async move {
                let http = Http::new(&bot_token_clone);
                let channel = ChannelId::new(chat_link_channel_id);
                while let Some(line) = rx.recv().await {
                    if let Err(e) = channel.say(&http, line).await {
                        error!("Couldn't send message to Discord: {:?}", e);
                    }
                }
            });
            DiscordClient::create_chat_link_g2d_subscriber(chat_link_tx.clone(), event_broker)
                .await;
        }

        let alert_tx;
        let alert_channel_http;
        if let Some(alert_channel_id) = alert_channel_id {
            let bot_token_clone = bot_token.clone();
            let (alert_tx_inner, mut rx) = mpsc::unbounded_channel();
            alert_tx = Some(alert_tx_inner);
            alert_channel_http = Some(Http::new(&bot_token_clone));
            tokio::spawn(async move {
                let http = Http::new(&bot_token_clone);
                let channel = ChannelId::new(alert_channel_id);
                while let Some(message) = rx.recv().await {
                    if let Err(e) = channel.say(&http, message).await {
                        error!("Couldn't send message to Discord: {:?}", e);
                    }
                }
            });
        } else {
            alert_tx = None;
            alert_channel_http = None;
        }

        Ok(DiscordClient {
            alert_tx,
            alert_channel_http,
            alert_channel_id,
            cache,
            _jh: jh,
        })
    }

    /// Returns a mapping from snowflake id to username#discriminator
    pub async fn get_user_list(&self) -> Result<HashMap<String, String>> {
        if let Some(http) = &self.alert_channel_http {
            let channel = ChannelId::new(self.alert_channel_id.unwrap());
            let ch = channel.to_channel((&self.cache, http)).await?;
            match ch.guild() {
                Some(g) => {
                    let members = http.get_guild_members(g.guild_id, None, None).await?;
                    let not_bots = members.into_iter().filter(|m| !m.user.bot);
                    Ok(not_bots
                        .map(|m| {
                            (
                                m.user.id.to_string(),
                                if let Some(discriminator) = m.user.discriminator {
                                    format!("{}#{:04}", m.user.name, discriminator)
                                } else {
                                    m.user.name
                                }
                            )
                        })
                        .collect())
                }
                None => {
                    error!("Only guild channels are supported for alerting");
                    Err(Error::Misconfiguration("Discord alerting is enabled, but a non-guild channel id was specified which is unsupported".to_owned()))
                }
            }
        } else {
            Err(Error::DiscordAlertingDisabled)
        }
    }

    pub fn oneshot_alert(&self, target_id: Option<String>, alert_msg: String) -> Result<()> {
        let mut mb = MessageBuilder::new();
        mb.push("**ALERT**");
        if let Some(target_id) = target_id {
            match target_id.parse() {
                Ok(target_id) => {
                    if let Some(tx) = &self.alert_tx {
                        let message = mb
                            .push(" for ")
                            .mention(&UserId::new(target_id))
                            .push(": ")
                            .push(alert_msg)
                            .build();
                        if let Err(e) = tx.send(message) {
                            error!("Error sending alert line through mpsc channel: {:?}", e);
                            Err(Error::InternalMessaging("Failed to send alert".to_owned()))
                        } else {
                            Ok(())
                        }
                    } else {
                        Err(Error::DiscordAlertingDisabled)
                    }
                }
                Err(_) => {
                    error!("Invalid target id");
                    Err(Error::BadRequest("Invalid target id".to_owned()))
                }
            }
        } else {
            if let Some(tx) = &self.alert_tx {
                let message = mb.push(": ").push(alert_msg).build();
                if let Err(e) = tx.send(message) {
                    error!("Error sending alert line through mpsc channel: {:?}", e);
                    Err(Error::InternalMessaging("Failed to send alert".to_owned()))
                } else {
                    Ok(())
                }
            } else {
                Err(Error::DiscordAlertingDisabled)
            }
        }
    }

    async fn create_chat_link_g2d_subscriber(
        send_msg_tx: mpsc::UnboundedSender<String>,
        event_broker: Arc<EventBroker>,
    ) {
        let chat_tx = send_msg_tx.clone();
        let join_tx = send_msg_tx.clone();
        let leave_tx = send_msg_tx;

        let chat_sub = event_broker
            .subscribe(TopicName(CHAT_TOPIC_NAME.to_string()), |_| true)
            .await;
        tokio::spawn(async move {
            pin_mut!(chat_sub);
            while let Some(event) = chat_sub.next().await {
                let message = event
                    .tags
                    .get(&TopicName(CHAT_TOPIC_NAME.to_string()))
                    .unwrap();
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
                let user = event
                    .tags
                    .get(&TopicName(JOIN_TOPIC_NAME.to_string()))
                    .unwrap();
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
                let user = event
                    .tags
                    .get(&TopicName(LEAVE_TOPIC_NAME.to_string()))
                    .unwrap();
                let message = format!("**{} has left the server**", user);
                if let Err(e) = leave_tx.send(message) {
                    error!("Error sending line through mpsc channel: {:?}", e);
                    break;
                }
            }

            error!(
                "Discord chat link g2d leave subscriber is finishing, this should never happen!"
            );
        });
    }
}

struct Handler {
    agent_client: Arc<AgentApiClient>,
    listen_channel_id: u64,
}

#[serenity::async_trait]
impl EventHandler for Handler {
    async fn message(&self, _ctx: Context, msg: Message) {
        if msg.channel_id == self.listen_channel_id && !msg.author.bot {
            // TODO indicate if it's a reply
            // TODO handle empty messages with embeds, attachments, etc
            let message_text = format!("{}: {}", msg.author.name, msg.content);
            let message_text = message_text.replace('\\', "\\\\");
            let message_text = message_text.replace('\'', "\\'");
            let command = format!("/silent-command game.print('[Discord] {}')", message_text);
            if let Err(e) = self.agent_client.rcon_command(command).await {
                error!(
                    "Couldn't send message via agent_client rcon_command: {:?}",
                    e
                );
            }
        }
    }

    async fn ready(&self, ctx: Context, _ready: Ready) {
        // update presence info with server status every 15 seconds
        let agent_client = Arc::clone(&self.agent_client);
        tokio::spawn(async move {
            loop {
                match agent_client.server_status().await {
                    Ok(ss) => {
                        let formatted = match ss {
                            ServerStatus::NotRunning
                            | ServerStatus::PreGame
                            | ServerStatus::PostGame => "Server offline".to_owned(),
                            ServerStatus::InGame { player_count } => {
                                format!("{} players online", player_count)
                            }
                        };
                        let activity = ActivityData::custom(formatted);
                        ctx.set_activity(Some(activity));
                    }
                    Err(e) => warn!(
                        "Error querying server status to update Discord presence: {}",
                        e
                    ),
                }
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        });

        info!("Discord event handler ready");
    }
}
