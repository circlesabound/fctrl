use std::sync::Arc;

use log::{debug, error};
use tokio::{net::ToSocketAddrs, sync::Mutex};

use crate::error::*;

pub struct Rcon {
    connection: Arc<Mutex<rcon::Connection>>,
}

impl Rcon {
    pub async fn connect<'a, T: ToSocketAddrs>(address: T, password: &'a str) -> Result<Rcon> {
        let connection = rcon::Connection::builder()
            .enable_factorio_quirks(true)
            .connect(address, password)
            .await?;
        let connection = Arc::new(Mutex::new(connection));
        Ok(Rcon { connection })
    }

    pub async fn send<'a>(&self, cmd: &'a str) -> Result<String> {
        // There is a bug with either the RCON library or with Factorio:
        // If we send an empty string, Factorio will not respond, and RCON will wait forever
        // Catch this case here
        if cmd.is_empty() {
            return Err(Error::RconEmptyCommand);
        }

        let mut mg = self.connection.lock().await;
        debug!("Sending command to RCON: '{}'", cmd);
        match mg.cmd(cmd).await {
            Ok(r) => {
                debug!("Got RCON response: '{}'", r);
                Ok(r)
            }
            Err(e) => {
                error!("Got RCON error: {:?}", e);
                Err(e.into())
            }
        }
    }
}
