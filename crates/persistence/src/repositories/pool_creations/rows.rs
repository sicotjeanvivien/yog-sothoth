use chrono::{DateTime, Utc};
use yog_core::{
    RepositoryError,
    domain::{PoolCreations, ProtocolBucket, ProtocolPoolCreations},
};

use crate::repositories::helper::{parse_bucket_from, parse_protocol};

pub(super) struct PoolCreationsRow {
    pub(super) protocol: String,
    pub(super) bucket_from: Option<DateTime<Utc>>,
    pub(super) pools: i64,
}

impl TryFrom<PoolCreationsRow> for ProtocolPoolCreations {
    type Error = RepositoryError;

    fn try_from(row: PoolCreationsRow) -> Result<Self, Self::Error> {
        Ok(ProtocolPoolCreations {
            protocol: parse_protocol(&row.protocol)?,
            pools: row.pools,
        })
    }
}

impl TryFrom<PoolCreationsRow> for ProtocolBucket<PoolCreations> {
    type Error = RepositoryError;

    fn try_from(row: PoolCreationsRow) -> Result<Self, Self::Error> {
        Ok(ProtocolBucket {
            protocol: parse_protocol(&row.protocol)?,
            from: parse_bucket_from(row.bucket_from)?,
            value: PoolCreations { pools: row.pools },
        })
    }
}
