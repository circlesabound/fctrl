use std::collections::HashMap;
use std::sync::Arc;

use log::error;
use serde::Deserialize;

use crate::clients::AgentApiClient;
use crate::db::{Cf, Db, Record};
use crate::discord::DiscordClient;
use crate::error::{Error, Result};
use crate::metrics::{DataPoint, MetricPeriod, Tick, METRIC_CF_NAME};

pub struct RpcHandler {
    agent_client: Arc<AgentApiClient>,
    db: Arc<Db>,
    discord: Arc<Option<DiscordClient>>,
}

impl RpcHandler {
    pub fn new(agent_client: Arc<AgentApiClient>, db: Arc<Db>, discord: Arc<Option<DiscordClient>>) -> RpcHandler {
        RpcHandler { agent_client, db, discord }
    }

    pub async fn handle(&self, command: &str) -> Result<()> {
        let (command, args) = command
            .split_once(' ')
            .ok_or(Error::Rpc("unable to extract rpc command".to_owned()))?;
        match command {
            "query" => {
                // args is <query_target>
                match args {
                    "discord" => {
                        // querying discord users
                        if let Some(d) = &*self.discord {
                            let users = d.get_user_list().await?;
                            // format as a lua table
                            let table = users
                                .into_iter()
                                .map(|(k, v)| format!("[\"{}\"]=\"{}\"", k, v))
                                .collect::<Vec<String>>()
                                .join(",");
                            let table = format!("{{{}}}", table);
                            
                            let remote_call = format!("/silent-command remote.call(\"fctrl-observers\", \"set_discord_users\", {})", table);
                            if let Err(e) = self.agent_client.rcon_command(remote_call).await {
                                Err(Error::Rpc(format!("error with rpc query discord callback: {:?}", e)))
                            } else {
                                Ok(())
                            }
                        } else {
                            Err(Error::Rpc(format!("discord integration not enabled")))
                        }
                    }
                    _ => Err(Error::Rpc(format!("invalid query target '{}'", args))),
                }
            }
            "oneshot" => {
                // args is a json string
                // Parse from json
                let oneshot = serde_json::from_str::<OneshotData>(args)?;
                if let Some(discord) = &*self.discord {
                    discord.oneshot_alert(oneshot.notif_target_id, format!("({},{}) {}", oneshot.position.x, oneshot.position.y, oneshot.message))
                } else {
                    Err(Error::Rpc(format!("discord integration not enabled")))
                }
            }
            "stream" => {
                // args is a json string
                // Parse batch from json
                let batch = serde_json::from_str::<DataPointBatch>(args)?;
                // Build data points
                let data_points = batch.data.iter().map(|(name, value)| {
                    match DataPoint::new(name.to_string(), MetricPeriod::PT05S, Tick(batch.timestamp), *value) {
                        Ok(data_point) => {
                            Some(data_point)
                        },
                        Err(e) => {
                            error!("Unable to construct data point for name {} timestamp {} value {}: {:?}", name, batch.timestamp, value, e);
                            None
                        },
                    }
                }).filter(|opt| opt.is_some()).map(|some| some.unwrap());
                // Build records
                let records = data_points.map(|dp| Record {
                    key: dp.key(),
                    value: dp.value().to_string(),
                });
                // Insert records into db
                let cf = Cf(METRIC_CF_NAME.to_string());
                for record in records {
                    if let Err(e) = self.db.write(&cf, &record) {
                        error!(
                            "Unable to write data point into db. Error: {:?}. Record: {:?}",
                            e, record
                        );
                    }
                }

                Ok(())
            }
            _ => Err(Error::Rpc(format!("invalid rpc command '{}'", command))),
        }
    }
}

/// This is what is streamed by the agent every stream interval
#[derive(Deserialize)]
struct DataPointBatch {
    /// Game tick
    timestamp: u64,
    /// Mapping from metric name (stream key) to data point value
    data: HashMap<String, f64>,
}

#[derive(Deserialize)]
struct OneshotData {
    /// Identifier representing who to notify (discord snowflake id)
    notif_target_id: Option<String>,
    /// Map position
    position: Position,
    /// Alert message
    message: String,
}

#[derive(Deserialize)]
struct Position {
    pub x: f64,
    pub y: f64,
}
