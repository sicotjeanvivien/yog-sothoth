use chrono::{DateTime, Utc};
use solana_signature::Signature;

use crate::domain::{
    MeteoraDammV2ClosePositionEvent, MeteoraDammV2CreatePositionEvent,
    MeteoraDammV2LockPositionEvent, MeteoraDammV2PermanentLockPositionEvent,
    MeteoraDammV2SplitPositionEvent, Protocol,
};

/// A position opened, closed, locked or split, under its protocol's own event.
#[derive(Debug, Clone)]
pub enum PositionChangeEvent {
    MeteoraDammV2(MeteoraDammV2PositionChange),
}

impl PositionChangeEvent {
    pub fn protocol(&self) -> Protocol {
        match self {
            Self::MeteoraDammV2(_) => Protocol::MeteoraDammV2,
        }
    }
}

/// A DAMM v2 position change: one of the five events that change a position.
#[derive(Debug, Clone)]
pub enum MeteoraDammV2PositionChange {
    Open(MeteoraDammV2CreatePositionEvent),
    Close(MeteoraDammV2ClosePositionEvent),
    VestingLock(MeteoraDammV2LockPositionEvent),
    PermanentLock(MeteoraDammV2PermanentLockPositionEvent),
    Split(Box<MeteoraDammV2SplitPositionEvent>),
}

impl MeteoraDammV2PositionChange {
    /// The key its list is sorted on: its instant, signature and index in the transaction.
    pub fn key(&self) -> (DateTime<Utc>, Signature, u16) {
        match self {
            Self::Open(e) => (e.timestamp, e.signature, e.event_index),
            Self::Close(e) => (e.timestamp, e.signature, e.event_index),
            Self::VestingLock(e) => (e.timestamp, e.signature, e.event_index),
            Self::PermanentLock(e) => (e.timestamp, e.signature, e.event_index),
            Self::Split(e) => (e.timestamp, e.signature, e.event_index),
        }
    }

    pub fn slot(&self) -> u64 {
        match self {
            Self::Open(e) => e.slot,
            Self::Close(e) => e.slot,
            Self::VestingLock(e) => e.slot,
            Self::PermanentLock(e) => e.slot,
            Self::Split(e) => e.slot,
        }
    }

    pub fn pool_address(&self) -> solana_pubkey::Pubkey {
        match self {
            Self::Open(e) => e.pool_address,
            Self::Close(e) => e.pool_address,
            Self::VestingLock(e) => e.pool_address,
            Self::PermanentLock(e) => e.pool_address,
            Self::Split(e) => e.pool_address,
        }
    }
}
