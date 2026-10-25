//! The measures yog-analytic writes, by protocol and by token: every read
//! covers a period ending at the close of the last frozen hour, the same for all.

use std::sync::Arc;

use chrono::TimeDelta;
use yog_core::{
    Page, RepositoryResult,
    domain::{
        FrozenHours, FrozenPeriod, LatestProtocolTvl, LiquidityMove, Protocol, ProtocolActivity,
        ProtocolActivityBar, ProtocolActivityLookup, ProtocolTvl, TokenActivity,
        TokenActivityCursor, TokenActivityFeed, Window,
    },
};

/// What a read found over its period; `period` is `None` before the first freeze.
#[derive(Debug, Clone)]
pub(crate) struct Measured<T> {
    pub(crate) period: Option<FrozenPeriod>,
    pub(crate) items: T,
}

/// A page of tokens, or the refusal of a cursor taken in a ranking since rewritten.
#[derive(Debug)]
pub(crate) enum TokenActivityRead {
    Page(Measured<Page<TokenActivity>>),
    CursorExpired,
}

pub(crate) struct ActivityService {
    protocols: Arc<dyn ProtocolActivityLookup>,
    tokens: Arc<dyn TokenActivityFeed>,
}

impl ActivityService {
    pub(crate) fn new(
        protocols: Arc<dyn ProtocolActivityLookup>,
        tokens: Arc<dyn TokenActivityFeed>,
    ) -> Self {
        Self { protocols, tokens }
    }

    async fn period(
        &self,
        window: Window,
        offset: Option<Window>,
    ) -> RepositoryResult<Option<FrozenPeriod>> {
        Ok(self
            .protocols
            .frozen_hours()
            .await?
            .map(|hours| FrozenPeriod::of(hours, window, offset)))
    }

    /// Totals: nothing for a period the facts do not cover whole.
    pub(crate) async fn protocol_activity(
        &self,
        window: Window,
        offset: Option<Window>,
    ) -> RepositoryResult<Measured<Vec<ProtocolActivity>>> {
        let period = self.period(window, offset).await?;
        let items = match &period {
            Some(period) if period.is_covered() => self.protocols.activity(period, window).await?,
            _ => Vec::new(),
        };
        Ok(Measured { period, items })
    }

    pub(crate) async fn protocol_activity_hourly(
        &self,
        window: Window,
    ) -> RepositoryResult<Measured<Vec<ProtocolActivityBar>>> {
        let period = self.period(window, None).await?;
        let items = match &period {
            Some(period) => self.protocols.activity_hourly(period).await?,
            None => Vec::new(),
        };
        Ok(Measured { period, items })
    }

    pub(crate) async fn protocol_activity_daily(
        &self,
        window: Window,
    ) -> RepositoryResult<Measured<Vec<ProtocolActivityBar>>> {
        let period = self.period(window, None).await?;
        let items = match &period {
            Some(period) => self.protocols.activity_daily(period).await?,
            None => Vec::new(),
        };
        Ok(Measured { period, items })
    }

    /// Each protocol's last TVL measured by the last frozen hour; its period is a
    /// day, of which only `as_of` counts.
    pub(crate) async fn protocol_tvl(&self) -> RepositoryResult<Measured<Vec<LatestProtocolTvl>>> {
        let period = self.period(Window::Day, None).await?;
        let items = match &period {
            Some(period) => self.protocols.tvl_latest(period.last_hour()).await?,
            None => Vec::new(),
        };
        Ok(Measured { period, items })
    }

    /// The TVL at the close of every hour from `period.from` through `period.as_of`:
    /// the first point is the window's start.
    pub(crate) async fn protocol_tvl_hourly(
        &self,
        window: Window,
    ) -> RepositoryResult<Measured<Vec<ProtocolTvl>>> {
        let period = self.period(window, None).await?;
        let items = match &period {
            Some(period) => {
                self.protocols
                    .tvl_hourly(period.from - TimeDelta::hours(1), period.last_hour())
                    .await?
            }
            None => Vec::new(),
        };
        Ok(Measured { period, items })
    }

    /// A protocol's largest valued liquidity moves over the window; nothing for a period its
    /// liquidity does not cover whole, which the repository reads.
    pub(crate) async fn liquidity_moves(
        &self,
        protocol: Protocol,
        window: Window,
        limit: i64,
    ) -> RepositoryResult<Measured<Vec<LiquidityMove>>> {
        let period = self.period(window, None).await?;
        let items = match &period {
            Some(period) => {
                self.protocols
                    .liquidity_moves(period, protocol, limit)
                    .await?
            }
            None => Vec::new(),
        };
        Ok(Measured { period, items })
    }

    /// The period ends at the hour of the ranking read, not at a separate read of
    /// the last frozen one; nothing for a period the facts do not cover whole.
    pub(crate) async fn token_activity(
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
        let period = self.protocols.frozen_hours().await?.map(|hours| {
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
}

#[cfg(test)]
#[path = "tests/activity_service_tests.rs"]
mod tests;
