//! The swaps' measures over a frozen period: for every protocol, one protocol, or one pool; and
//! the tokens by volume.

use std::sync::Arc;

use solana_pubkey::Pubkey;
use yog_core::{
    Page, RepositoryResult,
    domain::{
        FrozenHours, FrozenPeriod, PoolSwapBucket, Protocol, ProtocolActivityLookup,
        ProtocolBucket, ProtocolSwapActivity, SwapActivity, SwapActivityLookup, TokenActivity,
        TokenActivityCursor, TokenActivityFeed, Window,
    },
};

use super::period::{BucketPeriod, Measured, build_frozen_period};

/// A page of tokens, or the refusal of a cursor taken in a ranking since rewritten.
#[derive(Debug)]
pub(crate) enum TokenActivityRead {
    Page(Measured<Page<TokenActivity>>),
    CursorExpired,
}

pub(crate) struct SwapActivityService {
    hours: Arc<dyn ProtocolActivityLookup>,
    swap_activity: Arc<dyn SwapActivityLookup>,
    tokens: Arc<dyn TokenActivityFeed>,
}

impl SwapActivityService {
    pub(crate) fn new(
        hours: Arc<dyn ProtocolActivityLookup>,
        swap_activity: Arc<dyn SwapActivityLookup>,
        tokens: Arc<dyn TokenActivityFeed>,
    ) -> Self {
        Self {
            hours,
            swap_activity,
            tokens,
        }
    }

    /// The tokens by volume. The period ends at the hour of the ranking read, not at a separate
    /// read of the last frozen one; nothing for a period the facts do not cover whole.
    pub(crate) async fn list_token_activity(
        &self,
        window: Window,
        cursor: Option<TokenActivityCursor>,
        limit: i64,
    ) -> RepositoryResult<TokenActivityRead> {
        let ranked_at = cursor.as_ref().map(|c| c.last_hour);
        let Some(read) = self.tokens.list(window, cursor, limit).await? else {
            return Ok(TokenActivityRead::Page(Measured {
                period: None,
                items: Page::empty(),
            }));
        };
        if ranked_at.is_some_and(|hour| hour != read.last_hour) {
            return Ok(TokenActivityRead::CursorExpired);
        }
        let period = self.hours.frozen_hours().await?.map(|hours| {
            FrozenPeriod::of(
                FrozenHours {
                    first: hours.first,
                    last_frozen: read.last_hour,
                },
                window,
                None,
            )
        });
        let items = match &period {
            Some(period) if period.is_covered() => read.page,
            _ => Page::empty(),
        };
        Ok(TokenActivityRead::Page(Measured { period, items }))
    }

    /// Totals: nothing for a period the facts do not cover whole.
    pub(crate) async fn get_swap_totals(
        &self,
        window: Window,
        offset: Option<Window>,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Measured<Vec<ProtocolSwapActivity>>> {
        let period = build_frozen_period(self.hours.as_ref(), window, offset).await?;
        let items = match &period {
            Some(period) if period.is_covered() => {
                self.swap_activity
                    .find_protocol_totals(period, window, protocol)
                    .await?
            }
            _ => Vec::new(),
        };
        Ok(Measured { period, items })
    }

    pub(crate) async fn list_swap_buckets(
        &self,
        window: Window,
        bucket: BucketPeriod,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Measured<Vec<ProtocolBucket<SwapActivity>>>> {
        let period = build_frozen_period(self.hours.as_ref(), window, None).await?;
        let items = match (&period, bucket) {
            (Some(period), BucketPeriod::Hour) => {
                self.swap_activity
                    .list_protocol_hourly(period, protocol)
                    .await?
            }
            (Some(period), BucketPeriod::Day) => {
                self.swap_activity
                    .list_protocol_daily(period, protocol)
                    .await?
            }
            (None, _) => Vec::new(),
        };
        Ok(Measured { period, items })
    }

    pub(crate) async fn get_pool_swap_totals(
        &self,
        window: Window,
        pool: Pubkey,
    ) -> RepositoryResult<Measured<Vec<SwapActivity>>> {
        let period = build_frozen_period(self.hours.as_ref(), window, None).await?;
        let items = match &period {
            Some(period) if period.is_covered() => self
                .swap_activity
                .find_pool_totals(period, pool)
                .await?
                .into_iter()
                .collect(),
            _ => Vec::new(),
        };
        Ok(Measured { period, items })
    }

    pub(crate) async fn list_pool_swap_buckets(
        &self,
        window: Window,
        bucket: BucketPeriod,
        pool: Pubkey,
    ) -> RepositoryResult<Measured<Vec<PoolSwapBucket>>> {
        let period = build_frozen_period(self.hours.as_ref(), window, None).await?;
        let items = match (&period, bucket) {
            (Some(period), BucketPeriod::Hour) => {
                self.swap_activity.list_pool_hourly(period, pool).await?
            }
            (Some(period), BucketPeriod::Day) => {
                self.swap_activity.list_pool_daily(period, pool).await?
            }
            (None, _) => Vec::new(),
        };
        Ok(Measured { period, items })
    }
}

#[cfg(test)]
#[path = "tests/swap_activity_service_tests.rs"]
mod tests;
