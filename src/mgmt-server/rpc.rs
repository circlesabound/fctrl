use std::collections::HashMap;
use std::sync::Arc;

use log::{error, warn};
use serde::Deserialize;

use crate::db::{Cf, Db, Record};
use crate::error::{Error, Result};
use crate::metrics::{DataPoint, MetricPeriod, Tick, METRIC_CF_NAME};

pub struct RpcHandler {
    db: Arc<Db>,
}

impl RpcHandler {
    pub fn new(db: Arc<Db>) -> RpcHandler {
        RpcHandler { db }
    }

    pub async fn handle(&self, command: &str) -> Result<()> {
        let (command, args) = command
            .split_once(' ')
            .ok_or(Error::Rpc("unable to extract rpc command".to_owned()))?;
        match command {
            "stream" => {
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
