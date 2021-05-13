#![feature(bool_to_option, decl_macro)]

use std::{io::Cursor, net::SocketAddr, path::PathBuf, sync::Arc};

use log::info;
use rocket::{async_trait, catchers, fairing::Fairing, routes};
use rocket_contrib::serve::StaticFiles;

use crate::{clients::AgentApiClient, events::broker::EventBroker, ws::WebSocketServer};

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

    let event_broker = Arc::new(EventBroker::new());

    let agent_addr = url::Url::parse(&std::env::var("AGENT_ADDR")?)?;
    info!("Creating agent client with address {}", agent_addr);
    let agent_client = AgentApiClient::new(agent_addr, Arc::clone(&event_broker)).await;

    let ws_port = std::env::var("MGMT_SERVER_WS_PORT")?.parse()?;
    let ws_addr = std::env::var("MGMT_SERVER_WS_ADDRESS")?.parse()?;
    let ws_bind = SocketAddr::new(ws_addr, ws_port);
    info!("Opening ws server at {}", ws_bind);
    let ws = WebSocketServer::new(ws_bind).await?;

    rocket::build()
        .attach(CORS::new())
        .manage(event_broker)
        .manage(agent_client)
        .manage(ws)
        .mount("/", routes![routes::options::options,])
        .mount(
            "/api/v0",
            routes![
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
        .mount("/", StaticFiles::from(get_dist_path()))
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

struct CORS {}

impl CORS {
    pub fn new() -> CORS {
        CORS {}
    }
}

#[async_trait]
impl Fairing for CORS {
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
