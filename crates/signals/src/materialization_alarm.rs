//! The alarm that fires when the continuous aggregates stop being materialised.

mod alarm;
mod metrics;
mod verdict;

pub(crate) use alarm::{MaterializationAlarm, MaterializationAlarmSettings};
pub(crate) use metrics::{AlarmMetrics, HEARTBEAT_FAILURES};
