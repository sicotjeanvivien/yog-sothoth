//! The measures yog-analytic writes, by protocol and by token: every read
//! covers a span ending at the close of the last frozen hour, the same for all.

use std::sync::Arc;

use chrono::TimeDelta;
use yog_core::{
    Page, RepositoryResult,
    domain::{
        FrozenHours, LatestProtocolTvl, ProtocolActivity, ProtocolActivityBar,
        ProtocolActivityLookup, ProtocolTvl, Span, TokenActivity, TokenActivityCursor,
        TokenActivityFeed, Window,
    },
};

/// What a read found over its span; `span` is `None` before the first freeze.
#[derive(Debug, Clone)]
pub(crate) struct Measured<T> {
    pub(crate) span: Option<Span>,
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

    async fn span(&self, window: Window, offset: Option<Window>) -> RepositoryResult<Option<Span>> {
        Ok(self
            .protocols
            .frozen_hours()
            .await?
            .map(|hours| Span::of(hours, window, offset)))
    }

    /// Totals: nothing for a span the facts do not cover whole.
    pub(crate) async fn protocol_activity(
        &self,
        window: Window,
        offset: Option<Window>,
    ) -> RepositoryResult<Measured<Vec<ProtocolActivity>>> {
        let span = self.span(window, offset).await?;
        let items = match &span {
            Some(span) if span.is_covered() => self.protocols.activity(span, window).await?,
            _ => Vec::new(),
        };
        Ok(Measured { span, items })
    }

    pub(crate) async fn protocol_activity_hourly(
        &self,
        window: Window,
    ) -> RepositoryResult<Measured<Vec<ProtocolActivityBar>>> {
        let span = self.span(window, None).await?;
        let items = match &span {
            Some(span) => self.protocols.activity_hourly(span).await?,
            None => Vec::new(),
        };
        Ok(Measured { span, items })
    }

    pub(crate) async fn protocol_activity_daily(
        &self,
        window: Window,
    ) -> RepositoryResult<Measured<Vec<ProtocolActivityBar>>> {
        let span = self.span(window, None).await?;
        let items = match &span {
            Some(span) => self.protocols.activity_daily(span).await?,
            None => Vec::new(),
        };
        Ok(Measured { span, items })
    }

    /// Each protocol's last TVL measured by the last frozen hour; its span is a
    /// day, of which only `as_of` counts.
    pub(crate) async fn protocol_tvl(&self) -> RepositoryResult<Measured<Vec<LatestProtocolTvl>>> {
        let span = self.span(Window::Day, None).await?;
        let items = match &span {
            Some(span) => self.protocols.tvl_latest(span.last_hour()).await?,
            None => Vec::new(),
        };
        Ok(Measured { span, items })
    }

    /// The TVL at the close of every hour from `span.from` through `span.as_of`:
    /// the first point is the window's start.
    pub(crate) async fn protocol_tvl_hourly(
        &self,
        window: Window,
    ) -> RepositoryResult<Measured<Vec<ProtocolTvl>>> {
        let span = self.span(window, None).await?;
        let items = match &span {
            Some(span) => {
                self.protocols
                    .tvl_hourly(span.from - TimeDelta::hours(1), span.last_hour())
                    .await?
            }
            None => Vec::new(),
        };
        Ok(Measured { span, items })
    }

    /// The span ends at the hour of the ranking read, not at a separate read of
    /// the last frozen one; nothing for a span the facts do not cover whole.
    pub(crate) async fn token_activity(
        &self,
        window: Window,
        cursor: Option<TokenActivityCursor>,
        limit: i64,
    ) -> RepositoryResult<TokenActivityRead> {
        let ranked_at = cursor.as_ref().map(|c| c.last_hour);
        let Some(read) = self.tokens.list(window, cursor, limit).await? else {
            return Ok(TokenActivityRead::Page(Measured {
                span: None,
                items: Page::empty(),
            }));
        };
        if ranked_at.is_some_and(|hour| hour != read.last_hour) {
            return Ok(TokenActivityRead::CursorExpired);
        }
        let span = self.protocols.frozen_hours().await?.map(|hours| {
            Span::of(
                FrozenHours {
                    first: hours.first,
                    last_frozen: read.last_hour,
                },
                window,
                None,
            )
        });
        let items = match &span {
            Some(span) if span.is_covered() => read.page,
            _ => Page::empty(),
        };
        Ok(TokenActivityRead::Page(Measured { span, items }))
    }
}

#[cfg(test)]
#[path = "tests/activity_service_tests.rs"]
mod tests;
