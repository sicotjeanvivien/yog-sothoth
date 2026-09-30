//! The long-running loops: the signal engine — one poll loop per detector,
//! deduplication, persistence — and the alarm that fires when the continuous
//! aggregates stop being materialised, whose rule is in
//! [`materialization`](super::materialization).

mod materialization_alarm;
mod materialization_alarm_metrics;
mod signal_engine;
mod signal_engine_metrics;

pub(crate) use materialization_alarm::{
    MaterializationAlarm, MaterializationAlarmSettings, STATEMENT_TIMEOUT,
};
pub(crate) use materialization_alarm_metrics::{AlarmMetrics, HEARTBEAT_FAILURES};
pub(crate) use signal_engine::SignalEngine;
pub(crate) use signal_engine_metrics::EngineMetrics;
