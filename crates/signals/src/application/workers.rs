//! The long-running loops: the signal engine — one poll loop per detector,
//! deduplication, persistence — and the alarm that fires when the continuous
//! aggregates stop being materialised.

mod materialization_alarm;
mod materialization_alarm_metrics;
mod materialization_verdict;
mod signal_engine;
mod signal_engine_metrics;

pub(crate) use materialization_alarm::{MaterializationAlarm, MaterializationAlarmSettings};
pub(crate) use materialization_alarm_metrics::{AlarmMetrics, HEARTBEAT_FAILURES};
pub(crate) use signal_engine::SignalEngine;
pub(crate) use signal_engine_metrics::EngineMetrics;
