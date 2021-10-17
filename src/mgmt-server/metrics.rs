use std::{
    convert::{TryFrom, TryInto},
    fmt::Display,
};

use crate::{
    db::Cf,
    error::{Error, Result},
};

const METRICS_CF_PREFIX: &str = "metrics";

pub struct DataPoint {
    pub metric_name: String,
    pub period: MetricPeriod,
    pub tick: Tick,
    pub value: f64,
}

pub const _METRIC_PERIOD_AND_NAME_PREFIX: usize = 6 + 1 + MAX_METRIC_NAME_LENGTH;

const MAX_METRIC_NAME_LENGTH: usize = 44;
const MAX_TICK_STRING_LENGTH: usize = 12;

/// 12 digits
pub const MAX_TICK: u64 = 999999999999;

impl DataPoint {
    pub fn new(
        metric_name: String,
        period: MetricPeriod,
        tick: Tick,
        value: f64,
    ) -> Result<DataPoint> {
        DataPoint::validate_metric_name(&metric_name)?;
        if tick.0.to_string().len() > MAX_TICK_STRING_LENGTH {
            return Err(Error::MetricInvalidKey(format!(
                "Requested tick {} greater than maximum supported tick of {}",
                tick.0,
                "9".repeat(MAX_TICK_STRING_LENGTH)
            )));
        }

        Ok(DataPoint {
            metric_name,
            period,
            tick,
            value,
        })
    }

    pub fn validate_metric_name(metric_name: impl AsRef<str>) -> Result<()> {
        if metric_name.as_ref().contains("#") {
            return Err(Error::MetricInvalidKey(format!(
                "Metric name {} contains disallowed character '#'",
                metric_name.as_ref(),
            )));
        }
        if metric_name.as_ref().len() > MAX_METRIC_NAME_LENGTH {
            return Err(Error::MetricInvalidKey(format!(
                "Metric name {} longer than maximum supported length of {} bytes",
                metric_name.as_ref(),
                MAX_METRIC_NAME_LENGTH,
            )));
        }
        Ok(())
    }

    pub fn try_from(key: String, value: f64) -> Result<DataPoint> {
        if key.len() != 64 {
            return Err(Error::MetricInvalidKey(format!(
                "Given metric key '{}' has an incorrect length",
                key
            )));
        }

        // Get period
        let (period_str, key) = key.split_at(6);
        let period = period_str.trim_end_matches("#");
        let period = period.to_string().try_into()?;

        // Get metric name
        let (metric_name, key) = key.split_at(MAX_METRIC_NAME_LENGTH + 1);
        let metric_name = metric_name.trim_end_matches("#");

        // Get tick
        let tick_str = key;
        let tick_str = tick_str.trim_start_matches("T");
        if let Ok(tick_u64) = tick_str.parse() {
            let tick = Tick(tick_u64);

            DataPoint::new(metric_name.to_string(), period, tick, value)
        } else {
            Err(Error::MetricInvalidKey(format!(
                "Unable to parse tick value from '{}'",
                tick_str
            )))
        }
    }

    pub fn key(&self) -> String {
        get_lookup_key(&self.period, &self.metric_name, &self.tick)
    }
}

pub enum MetricPeriod {
    PT24H,
    PT12H,
    PT06H,
    PT01H,
    PT30M,
    PT05M,
    PT01M,
    PT30S,
    PT05S,
}

impl Display for MetricPeriod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            MetricPeriod::PT24H => "PT24H",
            MetricPeriod::PT12H => "PT12H",
            MetricPeriod::PT06H => "PT06H",
            MetricPeriod::PT01H => "PT01H",
            MetricPeriod::PT30M => "PT30M",
            MetricPeriod::PT05M => "PT05M",
            MetricPeriod::PT01M => "PT01M",
            MetricPeriod::PT30S => "PT30S",
            MetricPeriod::PT05S => "PT05S",
        };
        f.write_str(s)
    }
}

impl TryFrom<String> for MetricPeriod {
    type Error = Error;

    fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
        match value.as_ref() {
            "PT24H" => Ok(MetricPeriod::PT24H),
            "PT12H" => Ok(MetricPeriod::PT12H),
            "PT06H" => Ok(MetricPeriod::PT06H),
            "PT01H" => Ok(MetricPeriod::PT01H),
            "PT30M" => Ok(MetricPeriod::PT30M),
            "PT05M" => Ok(MetricPeriod::PT05M),
            "PT01M" => Ok(MetricPeriod::PT01M),
            "PT30S" => Ok(MetricPeriod::PT30S),
            "PT05S" => Ok(MetricPeriod::PT05S),
            _ => Err(Error::MetricInvalidKey(format!(
                "Invalid metric period {}",
                value
            ))),
        }
    }
}

pub struct Tick(pub u64);

impl Display for Tick {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!(
            "T{:0>max_tick_length$}",
            self.0,
            max_tick_length = MAX_TICK_STRING_LENGTH
        ))
    }
}
pub fn get_cf(period: &MetricPeriod) -> Cf {
    Cf(format!("{}_{}", METRICS_CF_PREFIX, period))
}

pub fn get_lookup_key(period: &MetricPeriod, metric_name: impl AsRef<str>, tick: &Tick) -> String {
    // Key is 64 length
    // Example key: 'PT30S#this-is-a-key################################T000000000300'
    format!("{}#{}#{}", period, pad_metric_name(metric_name), tick)
}

fn pad_metric_name(metric_name: impl AsRef<str>) -> String {
    let metric_name_len = metric_name.as_ref().len();
    let padding_required = MAX_METRIC_NAME_LENGTH - metric_name_len;
    let padding = "#".repeat(padding_required);
    format!("{}{}", metric_name.as_ref(), padding)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_pad_ascii_metric_name() -> std::result::Result<(), Box<dyn std::error::Error>> {
        fctrl::util::testing::logger_init();
        let name = "this is a normal name".to_owned();
        let padded = pad_metric_name(name);
        assert_eq!(padded.len(), 44);
        Ok(())
    }

    #[test]
    fn can_pad_unicode_metric_name() -> std::result::Result<(), Box<dyn std::error::Error>> {
        fctrl::util::testing::logger_init();
        let double_length = "é".to_owned();
        let padded = pad_metric_name(double_length);
        assert_eq!(padded.len(), 44);
        Ok(())
    }
}
