#![feature(bool_to_option, decl_macro)]

use std::{path::PathBuf, sync::Arc};

use log::{error, info};
use rocket::{catchers, routes};
use rocket_contrib::serve::StaticFiles;
use tokio::fs;

use crate::{clients::AgentApiClient, events::broker::EventBroker};

mod catchers;
mod clients;
mod consts;
mod error;
mod events;
mod routes;

#[rocket::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let event_broker = Arc::new(EventBroker::new());

    let agent_addr = url::Url::parse(&std::env::var("AGENT_ADDR")?)?;
    info!("Creating agent client with address {}", agent_addr);
    let agent_client = AgentApiClient::new(agent_addr, Arc::clone(&event_broker)).await;

    // test123().await;
    let _ = rocket::build()
        .manage(event_broker)
        .manage(agent_client)
        .mount(
            "/api/v0",
            routes![
                routes::server::upgrade_install,
                routes::server::status,
                routes::server::start_server,
                routes::server::stop_server,
                routes::server::get_savefiles,
            ],
        )
        .mount("/", StaticFiles::from(get_dist_path()))
        .register("/api/v0", catchers![catchers::not_found,])
        .register("/", catchers![catchers::fallback_to_index_html,])
        .launch()
        .await;

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

async fn _test123() -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(&*consts::DB_DIR).await?;
    info!("Opening db");
    let db = rocksdb::DB::open_default(consts::DB_DIR.join("testdb"))?;
    info!("Writing {{'key','hello this is value'}} to db");
    db.put(b"key", b"hello this is value")?;
    match db.get(b"key") {
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
