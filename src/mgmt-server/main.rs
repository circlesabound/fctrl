#![feature(bool_to_option, decl_macro)]

use std::{io::Cursor, net::SocketAddr, path::PathBuf, sync::Arc};

use auth::{AuthManager, AuthProvider};
use events::{
    TopicName, STDOUT_TOPIC_CHAT_CATEGORY, STDOUT_TOPIC_JOINLEAVE_CATEGORY, STDOUT_TOPIC_NAME,
    STDOUT_TOPIC_SYSTEMLOG_CATEGORY,
};
use futures::{pin_mut, StreamExt};
use log::{error, info};
use rocket::{async_trait, catchers, fairing::Fairing, fs::FileServer, routes};

use crate::{
    clients::AgentApiClient,
    db::{Cf, Db, Record},
    events::broker::EventBroker,
    ws::WebSocketServer,
};

mod auth;
mod catchers;
mod clients;
mod consts;
mod db;
mod error;
mod events;
mod guards;
mod routes;
mod ws;

#[rocket::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    info!("Creating event broker");
    let event_broker = Arc::new(EventBroker::new());

    info!("Opening db");
    let db = Arc::new(Db::open_or_new(&*consts::DB_DIR).await?);

    info!("Creating auth manager");
    let auth_provider = match &std::env::var("AUTH_PROVIDER")?.as_ref() {
        &"discord" => AuthProvider::Discord {
            client_id: std::env::var("AUTH_DISCORD_CLIENT_ID")?,
            client_secret: std::env::var("AUTH_DISCORD_CLIENT_SECRET")?,
        },
        &"none" => AuthProvider::None,
        other => {
            error!(
                "Invalid value '{}' for env var AUTH_PROVIDER, using auth provider None",
                other
            );
            AuthProvider::None
        }
    };
    let auth = AuthManager::new(auth_provider)?;

    let agent_addr = url::Url::parse(&std::env::var("AGENT_ADDR")?)?;
    info!("Creating agent client with address {}", agent_addr);
    let agent_client = AgentApiClient::new(agent_addr, Arc::clone(&event_broker)).await;

    info!("Creating db ingestion subscriber");
    create_db_ingestion_subscriber(Arc::clone(&event_broker), Arc::clone(&db)).await?;

    let ws_port = std::env::var("MGMT_SERVER_WS_PORT")?.parse()?;
    let ws_addr = std::env::var("MGMT_SERVER_WS_ADDRESS")?.parse()?;
    let ws_bind = SocketAddr::new(ws_addr, ws_port);
    info!("Opening ws server at {}", ws_bind);
    let ws = WebSocketServer::new(ws_bind).await?;

    rocket::build()
        .attach(Cors::new())
        .manage(auth)
        .manage(event_broker)
        .manage(db)
        .manage(agent_client)
        .manage(ws)
        .mount("/", routes![routes::options::options,])
        .mount(
            "/api/v0",
            routes![
                routes::auth::info,
                routes::auth::discord_grant,
                routes::auth::discord_refresh,
                routes::server::status,
                routes::server::start_server,
                routes::server::stop_server,
                routes::server::upgrade_install,
                routes::server::get_install,
                routes::server::get_savefiles,
                routes::server::create_savefile,
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
                routes::logs::get,
                routes::logs::stream,
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

async fn create_db_ingestion_subscriber(
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
