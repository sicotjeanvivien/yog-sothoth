use chrono::{DateTime, Utc};

use crate::domain::Protocol;

/// One protocol's measure over one bucket of a series, from `from`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolBucket<T> {
    pub protocol: Protocol,
    pub from: DateTime<Utc>,
    pub value: T,
}
