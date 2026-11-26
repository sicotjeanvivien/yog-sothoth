use crate::domain::Protocol;

/// How many pools of one protocol were ever recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtocolPools {
    pub protocol: Protocol,
    pub pools_seen: i64,
}
