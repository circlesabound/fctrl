use std::sync::Arc;

use fctrl::schema::mgmt_server_rest::{MetricsDataPoint, MetricsPaginationObject, MetricsPeriod};
use log::{debug, error};
use rocket::{get, serde::json::Json, State};

use crate::{
    db::{Db, RangeDirection},
    error::{Error, Result},
    metrics::{get_cf, get_lookup_key, DataPoint, MetricPeriod, Tick, MAX_TICK},
};

#[get("/metrics/<name>?<count>&<period>&<direction>&<from>")]
pub async fn get<'a>(
    db: &State<Arc<Db>>,
    name: String,
    period: String, // actually a MetricsPeriod
    count: u32,
    direction: String,
    from: Option<u64>,
) -> Result<Json<MetricsPaginationObject>> {
    // validate period
    if let Some(period) = try_parse_metrics_period(&period) {
        let cf = get_cf(&period);
        // validate metric name
        DataPoint::validate_metric_name(&name)?;

        let range_direction = match direction.to_lowercase().as_ref() {
            "forward" => Ok(RangeDirection::Forward),
            "backward" => Ok(RangeDirection::Backward),
            s => Err(Error::BadRequest(format!(
                "Invalid direction '{}', expected Forward or Backward",
                s
            ))),
        }?;

        let ret = match from {
            Some(from_key) => {
                let tick = Tick(from_key);
                let lookup_key = get_lookup_key(&period, &name, &tick);
                db.read_range(&cf, lookup_key, range_direction, count)?
            }
            None => match range_direction {
                RangeDirection::Forward => {
                    let tick = Tick(0);
                    let lookup_key = get_lookup_key(&period, &name, &tick);
                    db.read_range(&cf, lookup_key, RangeDirection::Forward, count)?
                }
                RangeDirection::Backward => {
                    let tick = Tick(MAX_TICK);
                    let lookup_key = get_lookup_key(&period, &name, &tick);
                    db.read_range(&cf, lookup_key, RangeDirection::Backward, count)?
                }
            },
        };

        let mut next = None;
        if let Some(k) = ret.continue_from {
            next = Some(DataPoint::try_from(k, 0.0)?.tick.0.to_string());
        }

        let mut datapoints = vec![];
        for r in ret.records.into_iter() {
            match r.value.parse::<f64>() {
                Ok(v) => {
                    match DataPoint::try_from(r.key, v) {
                        Ok(dp) => {
                            // ensure metric name is what we want
                            // if it is different, we've passed the start or end and can exit early
                            if dp.metric_name != name {
                                debug!("mismatch in metric name post db lookup");
                                next = None;
                                break;
                            } else {
                                // otherwise data point is valid and we can add it to the resposne
                                datapoints.push(dp);
                            }
                        }
                        Err(e) => {
                            error!(
                                "Failed to cast datapoint record to DataPoint object: {:?}",
                                e
                            );
                        }
                    }
                }
                Err(e) => {
                    error!(
                        "Failed to parse f64 value from datapoint record fetched from db: {:?}",
                        e
                    );
                }
            }
        }

        // Transform into the codegen'ed types
        let mdps = datapoints
            .into_iter()
            .map(|dp| MetricsDataPoint {
                tick: dp.tick.0 as i64,
                period: dp.period.into(),
                value: dp.value,
            })
            .collect();
        Ok(Json(MetricsPaginationObject {
            next,
            datapoints: mdps,
        }))
    } else {
        Err(Error::BadRequest(format!(
            "Invalid metric period {}",
            period
        )))
    }
}

fn try_parse_metrics_period(str: impl AsRef<str>) -> Option<MetricPeriod> {
    match str.as_ref() {
        "PT5S" => Some(MetricPeriod::PT05S),
        "PT30S" => Some(MetricPeriod::PT30S),
        "PT1M" => Some(MetricPeriod::PT01M),
        "PT5M" => Some(MetricPeriod::PT05M),
        "PT30M" => Some(MetricPeriod::PT30M),
        "PT1H" => Some(MetricPeriod::PT01H),
        "PT6H" => Some(MetricPeriod::PT06H),
        "PT12H" => Some(MetricPeriod::PT12H),
        "P1D" => Some(MetricPeriod::PT24H),
        _ => None,
    }
}

impl From<MetricPeriod> for MetricsPeriod {
    fn from(mp: MetricPeriod) -> Self {
        match mp {
            MetricPeriod::PT24H => MetricsPeriod::P1D,
            MetricPeriod::PT12H => MetricsPeriod::PT12H,
            MetricPeriod::PT06H => MetricsPeriod::PT6H,
            MetricPeriod::PT01H => MetricsPeriod::PT1H,
            MetricPeriod::PT30M => MetricsPeriod::PT30M,
            MetricPeriod::PT05M => MetricsPeriod::PT5M,
            MetricPeriod::PT01M => MetricsPeriod::PT1M,
            MetricPeriod::PT30S => MetricsPeriod::PT30S,
            MetricPeriod::PT05S => MetricsPeriod::PT5S,
        }
    }
}
