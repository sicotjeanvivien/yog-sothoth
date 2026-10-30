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

use super::period::{
    BucketPeriod, Measured, ReadPeriod, SeriesInterval, build_frozen_period, build_last_hour,
    build_read_period,
};

/// A page of tokens, or the refusal of a cursor taken in a ranking since rewritten.
#[derive(Debug)]
pub(crate) enum TokenActivityRead {
    Page(Measured<Page<TokenActivity>, ReadPeriod>),
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
        if window == Window::Hour {
            return self.list_token_activity_in_hour(cursor, limit).await;
        }
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
        Ok(TokenActivityRead::Page(Measured {
            period: period.map(ReadPeriod::Frozen),
            items,
        }))
    }

    /// The ranking of the last hour, read live: a cursor taken before a tranche was written is
    /// refused, its ranking having moved.
    async fn list_token_activity_in_hour(
        &self,
        cursor: Option<TokenActivityCursor>,
        limit: i64,
    ) -> RepositoryResult<TokenActivityRead> {
        let Some(period) = build_last_hour(self.hours.as_ref()).await? else {
            return Ok(TokenActivityRead::Page(Measured {
                period: None,
                items: Page::empty(),
            }));
        };
        if cursor.as_ref().is_some_and(|c| c.last_hour != period.as_of) {
            return Ok(TokenActivityRead::CursorExpired);
        }
        let items = if period.is_covered() {
            self.tokens
                .list_in_tranches(&period, cursor, limit)
                .await?
                .page
        } else {
            Page::empty()
        };
        Ok(TokenActivityRead::Page(Measured {
            period: Some(ReadPeriod::Tranche(period)),
            items,
        }))
    }

    /// Totals: nothing for a period the facts do not cover whole.
    pub(crate) async fn get_swap_totals(
        &self,
        window: Window,
        offset: Option<Window>,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Measured<Vec<ProtocolSwapActivity>, ReadPeriod>> {
        let period = build_read_period(self.hours.as_ref(), window, offset).await?;
        let items = match &period {
            Some(ReadPeriod::Frozen(p)) if p.is_covered() => {
                self.swap_activity
                    .find_protocol_totals(p, window, protocol)
                    .await?
            }
            Some(ReadPeriod::Tranche(p)) if p.is_covered() => {
                self.swap_activity
                    .find_protocol_totals_in_tranches(p, protocol)
                    .await?
            }
            _ => Vec::new(),
        };
        Ok(Measured { period, items })
    }

    /// The series of `window` at `interval`.
    pub(crate) async fn list_swap_series(
        &self,
        window: Window,
        interval: SeriesInterval,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Measured<Vec<ProtocolBucket<SwapActivity>>, ReadPeriod>> {
        let Some(bucket) = interval.frozen_bucket() else {
            let period = build_last_hour(self.hours.as_ref()).await?;
            let items = match &period {
                Some(p) => {
                    self.swap_activity
                        .list_protocol_tranches(p, protocol)
                        .await?
                }
                None => Vec::new(),
            };
            let period = period.map(ReadPeriod::Tranche);
            return Ok(Measured { period, items });
        };
        let measured = self.list_swap_buckets(window, bucket, protocol).await?;
        Ok(Measured {
            period: measured.period.map(ReadPeriod::Frozen),
            items: measured.items,
        })
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
            (Some(period), bucket) => {
                self.swap_activity
                    .list_protocol_buckets(period, bucket.width(), protocol)
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
    ) -> RepositoryResult<Measured<Vec<SwapActivity>, ReadPeriod>> {
        let period = build_read_period(self.hours.as_ref(), window, None).await?;
        let items = match &period {
            Some(ReadPeriod::Frozen(p)) if p.is_covered() => {
                self.swap_activity.find_pool_totals(p, pool).await?
            }
            Some(ReadPeriod::Tranche(p)) if p.is_covered() => {
                self.swap_activity
                    .find_pool_totals_in_tranches(p, pool)
                    .await?
            }
            _ => None,
        };
        Ok(Measured {
            period,
            items: items.into_iter().collect(),
        })
    }

    pub(crate) async fn list_pool_swap_series(
        &self,
        window: Window,
        interval: SeriesInterval,
        pool: Pubkey,
    ) -> RepositoryResult<Measured<Vec<PoolSwapBucket>, ReadPeriod>> {
        let period = build_read_period(self.hours.as_ref(), window, None).await?;
        let items = match (&period, interval.frozen_bucket()) {
            (Some(ReadPeriod::Tranche(p)), None) => {
                self.swap_activity.list_pool_tranches(p, pool).await?
            }
            (Some(ReadPeriod::Frozen(p)), Some(BucketPeriod::Hour)) => {
                self.swap_activity.list_pool_hourly(p, pool).await?
            }
            (Some(ReadPeriod::Frozen(p)), Some(bucket)) => {
                self.swap_activity
                    .list_pool_buckets(p, bucket.width(), pool)
                    .await?
            }
            _ => Vec::new(),
        };
        Ok(Measured { period, items })
    }
}

#[cfg(test)]
#[path = "tests/swap_activity_service_tests.rs"]
mod tests;
