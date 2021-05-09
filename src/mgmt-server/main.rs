#![feature(bool_to_option, decl_macro)]

use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use log::info;
use rocket::{catchers, routes};
use rocket_contrib::serve::StaticFiles;

use crate::{clients::AgentApiClient, events::broker::EventBroker, ws::WebSocketServer};

mod catchers;
mod clients;
mod consts;
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
        .manage(event_broker)
        .manage(agent_client)
        .manage(ws)
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

pub fn get_dist_path() -> PathBuf {
    std::env::current_dir()
        .unwrap()
        .join("web")
        .join("dist")
        .join("web")
}

#[cfg(test)]
mod tests {
    use super::*;

    use log::error;
    use tokio::fs;

    #[tokio::test]
    async fn test_rocksdb() -> std::result::Result<(), Box<dyn std::error::Error>> {
        fs::create_dir_all(&*consts::DB_DIR).await?;

        info!("Opening db");
        let db_path = consts::DB_DIR.join("testdb");
        let secondary_path = consts::DB_DIR.join("testdb_secondary");
        let db = rocksdb::DB::open_default(&db_path)?;

        info!("Opening secondary");
        let mut opts = rocksdb::Options::default();
        opts.set_max_open_files(-1);
        let db_r = rocksdb::DB::open_as_secondary(&opts, &db_path, &secondary_path)?;

        info!("Writing {{'key','hello this is value'}} to db");
        db.put(b"key", b"hello this is value")?;

        info!("Reading from secondary");
        db_r.try_catch_up_with_primary()?;

        match db_r.get(b"key") {
            Ok(Some(value)) => {
                info!(
                    "Retrieved written value from the db: {}",
                    String::from_utf8(value).unwrap()
                );
            }
            Ok(None) => {
                error!("Retrieved empty value from db");
            }
            Err(e) => {
                error!("Error retrieving value from db: {:?}", e)
            }
        }
        db.delete(b"key")?;
        Ok(())
    }
}
