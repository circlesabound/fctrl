#![feature(decl_macro)]

use std::path::PathBuf;

use log::{error, info};
use rocket::{catchers, routes};
use rocket_contrib::serve::StaticFiles;
use tokio::fs;

mod catchers;
mod consts;
mod routes;

#[rocket::main]
async fn main() {
    env_logger::init();

    test123().await;
    let _ = rocket::build()
        .mount(
            "/api",
            routes![
                routes::server::status,
                routes::server::start_server,
                routes::server::stop_server
            ],
        )
        .mount("/", StaticFiles::from(get_dist_path()))
        .register("/", catchers![catchers::not_found,])
        .launch()
        .await;
    info!("Shutting down");
}

fn get_dist_path() -> PathBuf {
    std::env::current_dir().unwrap().join("web").join("dist").join("web")
}

async fn test123() -> Result<(), Box<dyn std::error::Error>> {
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
