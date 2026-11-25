//! Unit tests for `TokenTvlService`: a cursor of a reading replaced since expires.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Duration, TimeZone, Utc};
use solana_pubkey::Pubkey;
use yog_core::{
    Page, RepositoryResult,
    domain::{
        Protocol, TokenTvlCursor, TokenTvlLookup, TokenTvlOrder, TokenTvlPage, TokenTvlTotal,
    },
};

use super::{TokenTvlRead, TokenTvlService};

fn reading() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 25, 12, 0, 0).unwrap()
}

/// A lens whose last reading is at `at`, or none.
struct Lens(Option<DateTime<Utc>>);

#[async_trait]
impl TokenTvlLookup for Lens {
    async fn list(
        &self,
        _: Protocol,
        _: TokenTvlOrder,
        _: Option<TokenTvlCursor>,
        _: i64,
    ) -> RepositoryResult<Option<TokenTvlPage>> {
        Ok(self.0.map(|at| TokenTvlPage {
            at,
            covered_since: None,
            total: TokenTvlTotal {
                tvl_usd: None,
                locked_usd: None,
            },
            page: Page::empty(),
        }))
    }
}

fn cursor(at: DateTime<Utc>) -> TokenTvlCursor {
    TokenTvlCursor {
        protocol: Protocol::MeteoraDammV2,
        at,
        order: TokenTvlOrder::Tvl,
        value: None,
        mint: Pubkey::new_unique(),
    }
}

async fn read(lens: Lens, cursor: Option<TokenTvlCursor>) -> TokenTvlRead {
    TokenTvlService::new(Arc::new(lens))
        .list_by_token(Protocol::MeteoraDammV2, TokenTvlOrder::Tvl, cursor, 10)
        .await
        .unwrap()
}

/// Mutation: the cursor's reading not compared, a key of the last hour ranking the next.
#[tokio::test]
async fn a_cursor_of_a_replaced_reading_expires() {
    let expired = read(
        Lens(Some(reading())),
        Some(cursor(reading() - Duration::hours(1))),
    )
    .await;
    assert!(matches!(expired, TokenTvlRead::CursorExpired));

    let gone = read(Lens(None), Some(cursor(reading()))).await;
    assert!(matches!(gone, TokenTvlRead::CursorExpired));

    let same = read(Lens(Some(reading())), Some(cursor(reading()))).await;
    assert!(matches!(same, TokenTvlRead::Page(Some(_))));
}
