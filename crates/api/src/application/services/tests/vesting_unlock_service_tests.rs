//! Unit tests for `VestingUnlockService`: a cursor of a reading replaced since expires.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Duration, TimeZone, Utc};
use solana_pubkey::Pubkey;
use yog_core::{
    Page, RepositoryResult,
    domain::{
        Protocol, VestingUnlockCursor, VestingUnlockLookup, VestingUnlockOrder, VestingUnlockPage,
        Window,
    },
};

use super::{VestingUnlockRead, VestingUnlockService};

fn reading() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 25, 12, 5, 0).unwrap()
}

/// A lens whose last reading is at `as_of`, or none.
struct Lens(Option<DateTime<Utc>>);

#[async_trait]
impl VestingUnlockLookup for Lens {
    async fn list(
        &self,
        _: Protocol,
        _: Window,
        _: VestingUnlockOrder,
        _: Option<VestingUnlockCursor>,
        _: i64,
    ) -> RepositoryResult<Option<VestingUnlockPage>> {
        Ok(self.0.map(|as_of| VestingUnlockPage {
            as_of,
            covered_since: None,
            page: Page::empty(),
        }))
    }
}

fn cursor(as_of: DateTime<Utc>) -> VestingUnlockCursor {
    VestingUnlockCursor {
        protocol: Protocol::MeteoraDammV2,
        as_of,
        window: Window::Week,
        order: VestingUnlockOrder::Time,
        amount_usd: None,
        unlocks_at: as_of + Duration::hours(1),
        pool_address: Pubkey::new_unique(),
        position: Pubkey::new_unique(),
    }
}

async fn read(lens: Lens, cursor: Option<VestingUnlockCursor>) -> VestingUnlockRead {
    VestingUnlockService::new(Arc::new(lens))
        .list(
            Protocol::MeteoraDammV2,
            Window::Week,
            VestingUnlockOrder::Time,
            cursor,
            10,
        )
        .await
        .unwrap()
}

/// Mutation: the cursor's reading not compared, a key of the last reading listing the next.
#[tokio::test]
async fn a_cursor_of_a_replaced_reading_expires() {
    let expired = read(
        Lens(Some(reading())),
        Some(cursor(reading() - Duration::hours(1))),
    )
    .await;
    assert!(matches!(expired, VestingUnlockRead::CursorExpired));

    let gone = read(Lens(None), Some(cursor(reading()))).await;
    assert!(matches!(gone, VestingUnlockRead::CursorExpired));

    let same = read(Lens(Some(reading())), Some(cursor(reading()))).await;
    assert!(matches!(same, VestingUnlockRead::Page(Some(_))));
}
