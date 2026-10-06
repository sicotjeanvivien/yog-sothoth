//! The long-running loops: the signal engine — one poll loop per detector,
//! deduplication, persistence — and the alarm that fires when the continuous
//! aggregates stop being materialised. Each is one file, and a folder of the
//! same name for what only it uses — the alarm's rule, its verdict and its
//! failures, among them.

mod materialization_alarm;
mod signal_engine;

pub(crate) use materialization_alarm::{
    AlarmMetrics, HEARTBEAT_FAILURES, MaterializationAlarm, MaterializationAlarmSettings,
    STATEMENT_TIMEOUT,
};
pub(crate) use signal_engine::{EngineMetrics, SignalEngine};
