#![feature(decl_macro)]

use log::{error, info};
use rocket::{catchers, get, routes};
use tokio::fs;

mod catchers;
mod consts;

#[rocket::main]
async fn main() {
    env_logger::init();

    test123().await;
    let _ = rocket::build()
        .mount("/hello", routes![world])
        .register("/", catchers![
            catchers::not_found,
        ])
        .launch()
        .await;
}

#[get("/world")]
async fn world() -> String {
    "test".to_owned()
}

async fn test123() -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(&*consts::DB_DIR).await?;
    info!("Opening db");
    let db = rocksdb::DB::open_default(consts::DB_DIR.join("testdb"))?;
    info!("Writing {{'key','hello this is value'}} to db");
    db.put(b"key", b"hello this is value")?;
    match db.get(b"key") {
        Ok(Some(value)) => {
            info!("Retrieved written value from the db: {}", String::from_utf8(value).unwrap());
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
