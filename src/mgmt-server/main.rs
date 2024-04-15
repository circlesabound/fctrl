#![feature(decl_macro)]

use std::{io::Cursor, net::SocketAddr, path::PathBuf, sync::Arc};

use auth::{AuthnManager, AuthnProvider, AuthzManager};
use events::{
    TopicName, RPC_TOPIC_NAME, STDOUT_TOPIC_CHAT_CATEGORY, STDOUT_TOPIC_JOINLEAVE_CATEGORY,
    STDOUT_TOPIC_NAME, STDOUT_TOPIC_SYSTEMLOG_CATEGORY,
};
use futures::{pin_mut, StreamExt};
use log::{debug, error, info};
use rocket::{async_trait, catchers, fairing::Fairing, fs::FileServer, routes};

use crate::{
    auth::UserIdentity, clients::AgentApiClient, db::{Cf, Db, Record}, discord::DiscordClient, events::broker::EventBroker, link_download::LinkDownloadManager, rpc::RpcHandler, ws::WebSocketServer
};

mod auth;
mod catchers;
mod clients;
mod consts;
mod db;
mod discord;
mod error;
mod events;
mod guards;
mod link_download;
mod metrics;
mod routes;
mod rpc;
mod ws;

#[rocket::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    info!("Creating event broker");
    let event_broker = Arc::new(EventBroker::new());

    info!("Opening db");
    let db = Arc::new(Db::open_or_new(&*consts::DB_DIR).await?);

    let agent_addr = url::Url::parse(&std::env::var("AGENT_ADDR")?)?;
    info!("Creating agent client with address {}", agent_addr);
    let agent_client = Arc::new(AgentApiClient::new(agent_addr, Arc::clone(&event_broker)).await);

    info!("Checking Discord integration...");
    let discord_client = Arc::new(match &std::env::var("DISCORD_INTEGRATION").as_deref() {
        Ok("true") => {
            info!("Discord integration enabled, setting up Discord client");
            let discord_bot_token = std::env::var("DISCORD_BOT_TOKEN")?;
            let alert_channel_id = match std::env::var("DISCORD_ALERT_CHANNEL_ID") {
                Ok(s) => Some(s.parse()?),
                Err(_) => None,
            };
            let chat_link_channel_id = match std::env::var("DISCORD_CHAT_LINK_CHANNEL_ID") {
                Ok(s) => Some(s.parse()?),
                Err(_) => None,
            };
            let chat_link_preserve_achievements = match std::env::var("DISCORD_CHAT_LINK_PRESERVE_ACHIEVEMENTS") {
                Ok(s) => s.parse()?,
                Err(_) => true,
            };
            Some(
                DiscordClient::new(
                    discord_bot_token,
                    alert_channel_id,
                    chat_link_channel_id,
                    chat_link_preserve_achievements,
                    Arc::clone(&agent_client),
                    Arc::clone(&event_broker),
                )
                .await?,
            )
        }
        _ => {
            info!("Discord integration disabled");
            None
        }
    });

    info!("Creating authn and authz manager");
    let auth_provider = match &std::env::var("AUTH_PROVIDER")?.as_ref() {
        &"discord" => {
            if discord_client.is_none() {
                return Err(error::Error::Misconfiguration(
                    "Authn provider selected as discord, but discord integration is disabled!"
                        .to_owned(),
                )
                .into());
            }
            AuthnProvider::Discord {
                client_id: std::env::var("DISCORD_OAUTH2_CLIENT_ID")?,
                client_secret: std::env::var("DISCORD_OAUTH2_CLIENT_SECRET")?,
            }
        }
        &"none" => AuthnProvider::None,
        other => {
            error!(
                "Invalid value '{}' for env var AUTH_PROVIDER, using auth provider None",
                other
            );
            AuthnProvider::None
        }
    };
    let authn = AuthnManager::new(auth_provider)?;
    let admin_user = match std::env::var("AUTH_DISCORD_ADMIN_USER_ID") {
        Ok(id) => UserIdentity { sub: id },
        Err(_) => UserIdentity::anonymous(),
    };
    let authz = AuthzManager::new(admin_user);

    info!("Creating log ingestion subscriber");
    create_log_ingestion_subscriber(Arc::clone(&event_broker), Arc::clone(&db)).await?;

    info!("Creating rpc subscriber");
    create_rpc_subscriber(
        Arc::clone(&agent_client),
        Arc::clone(&event_broker),
        Arc::clone(&db),
        Arc::clone(&discord_client),
    )
    .await?;

    info!("Creating link download manager");
    let link_download_manager = Arc::new(LinkDownloadManager::new().await);

    let ws_port = std::env::var("MGMT_SERVER_WS_PORT")?.parse()?;
    let ws_addr = std::env::var("MGMT_SERVER_WS_ADDRESS")?.parse()?;
    let ws_bind = SocketAddr::new(ws_addr, ws_port);
    let reverse_proxy_enabled: bool = std::env::var("RPROXY_ENABLED")?.parse()?;
    if reverse_proxy_enabled {
        info!("Env var suggests reverse proxy is enabled, will enable WSS");
    }
    info!("Opening websocket server at {}", ws_bind);
    let ws = WebSocketServer::new(ws_bind, reverse_proxy_enabled).await?;

    rocket::build()
        .attach(Cors::new())
        .manage(authn)
        .manage(authz)
        .manage(event_broker)
        .manage(db)
        .manage(agent_client)
        .manage(link_download_manager)
        .manage(ws)
        .mount("/", routes![routes::options::options,])
        .mount(
            "/api/v0",
            routes![
                routes::auth::info,
                routes::auth::discord_grant,
                routes::auth::discord_refresh,
                routes::server::status,
                routes::server::create_savefile,
                routes::server::start_server,
                routes::server::stop_server,
                routes::server::upgrade_install,
                routes::server::get_install,
                routes::server::get_savefile,
                routes::server::get_savefiles,
                routes::server::get_adminlist,
                routes::server::put_adminlist,
                routes::server::get_banlist,
                routes::server::put_banlist,
                routes::server::get_whitelist,
                routes::server::put_whitelist,
                routes::server::get_rcon_config,
                routes::server::put_rcon_config,
                routes::server::get_secrets,
                routes::server::put_secrets,
                routes::server::get_server_settings,
                routes::server::put_server_settings,
                routes::server::get_mods_list,
                routes::server::apply_mods_list,
                routes::server::get_mod_settings,
                routes::server::put_mod_settings,
                routes::server::get_mod_settings_dat,
                routes::server::put_mod_settings_dat,
                routes::server::send_rcon_command,
                routes::logs::get,
                routes::logs::stream,
                routes::metrics::get,
            ],
        )
        .mount(
            "/proxy",
            routes![
                routes::proxy::mod_portal_batch_get,
                routes::proxy::mod_portal_short_get,
                routes::proxy::mod_portal_full_get,
            ],
        )
        .mount(
            "/download",
            routes![
                routes::download::download,
            ]
        )
        .mount("/", FileServer::from(get_dist_path()))
        .register("/api/v0", catchers![catchers::not_found,])
        .register("/", catchers![catchers::fallback_to_index_html,])
        .launch()
        .await?;

    info!("Shutting down");

    Ok(())
}

fn get_dist_path() -> PathBuf {
    std::env::current_dir()
        .unwrap()
        .join("web")
        .join("dist")
        .join("web")
}

async fn create_log_ingestion_subscriber(
    event_broker: Arc<EventBroker>,
    db: Arc<Db>,
) -> crate::error::Result<()> {
    let stdout_sub = event_broker
        .subscribe(TopicName(STDOUT_TOPIC_NAME.to_string()), |_| true)
        .await;
    tokio::spawn(async move {
        pin_mut!(stdout_sub);
        while let Some(event) = stdout_sub.next().await {
            // Map to the right CF
            if let Some(tag_value) = event.tags.get(&TopicName(STDOUT_TOPIC_NAME.to_string())) {
                let opt_cf = match tag_value.as_str() {
                    STDOUT_TOPIC_CHAT_CATEGORY => Some(STDOUT_TOPIC_CHAT_CATEGORY),
                    STDOUT_TOPIC_JOINLEAVE_CATEGORY => Some(STDOUT_TOPIC_JOINLEAVE_CATEGORY),
                    STDOUT_TOPIC_SYSTEMLOG_CATEGORY => Some(STDOUT_TOPIC_SYSTEMLOG_CATEGORY),
                    _ => None,
                };

                if let Some(cf) = opt_cf {
                    let record = Record {
                        key: event.timestamp.to_rfc3339(),
                        value: event.content,
                    };
                    if let Err(e) = db.write(&Cf(cf.to_string()), &record) {
                        error!("Error writing to db: {:?}", e);
                    }
                }
            } else {
                error!("missing tag, this should never happen");
            }
        }

        error!("stdout ingestion subscriber task is finishing - this should never happen!");
    });

    Ok(())
}

async fn create_rpc_subscriber(
    agent_client: Arc<AgentApiClient>,
    event_broker: Arc<EventBroker>,
    db: Arc<Db>,
    discord: Arc<Option<DiscordClient>>,
) -> crate::error::Result<()> {
    let rpc_sub = event_broker
        .subscribe(TopicName(RPC_TOPIC_NAME.to_string()), |_| true)
        .await;
    tokio::spawn(async move {
        pin_mut!(rpc_sub);
        let rpc_handler = Arc::new(RpcHandler::new(agent_client, db, discord));
        while let Some(mut event) = rpc_sub.next().await {
            if let Some(command) = event.tags.remove(&TopicName(RPC_TOPIC_NAME.to_string())) {
                let rpc_handler = Arc::clone(&rpc_handler);
                tokio::spawn(async move {
                    debug!("handling rpc command: {}", command);
                    if let Err(e) = rpc_handler.handle(&command).await {
                        error!("error from rpc handler for command '{}': {:?}", command, e);
                    }
                });
            }
        }

        error!("rpc subscriber task is finishing - this should never happen!");
    });

    Ok(())
}

struct Cors {}

impl Cors {
    pub fn new() -> Cors {
        Cors {}
    }
}

#[async_trait]
impl Fairing for Cors {
    fn info(&self) -> rocket::fairing::Info {
        rocket::fairing::Info {
            name: "Add CORS headers to response",
            kind: rocket::fairing::Kind::Response,
        }
    }

    async fn on_response<'r>(&self, req: &'r rocket::Request<'_>, res: &mut rocket::Response<'r>) {
        res.set_header(rocket::http::Header::new(
            "Access-Control-Allow-Origin",
            "*",
        ));
        res.set_header(rocket::http::Header::new(
            "Access-Control-Allow-Methods",
            "GET, OPTIONS, POST, PUT",
        ));
        res.set_header(rocket::http::Header::new(
            "Access-Control-Allow-Headers",
            "*",
        ));
        res.set_header(rocket::http::Header::new(
            "Access-Control-Allow-Credentials",
            "false",
        ));
        res.set_header(rocket::http::Header::new(
            "Access-Control-Expose-Headers",
            "Location",
        ));

        if req.method() == rocket::http::Method::Options {
            res.set_header(rocket::http::ContentType::Plain);
            res.set_sized_body(0, Cursor::new(""))
        }
    }
}
