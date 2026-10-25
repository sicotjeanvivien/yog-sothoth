//! `PgTokenPriceRepository`'s list under `yog_api`: a mint's prices from the start of a
//! window, the most recent first, paged without repeat nor gap.

use chrono::{DateTime, Duration, TimeZone, Utc};
use sqlx::PgPool;
use yog_core::{Cursor, domain::TokenPriceLookup};
use yog_persistence::PgTokenPriceRepository;

use super::helpers::{pk, price_at, setup_roles_and_connect_as};

fn at(minutes: i64) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 20, 12, 0, 0).unwrap() + Duration::minutes(minutes)
}

/// Mutation: the mint filter or the window's start dropped — another mint's price, or one
/// before the window, listed; or the cursor compared with `<=` — a price repeated.
#[sqlx::test(migrations = "./migrations")]
async fn a_mints_prices_of_the_window_page_without_repeat_nor_gap(pool: PgPool) {
    let (mint, other) = (pk(1).to_string(), pk(2).to_string());
    for minutes in [1, 2, 3, 4, 5] {
        price_at(&pool, &mint, "1", at(minutes)).await;
    }
    price_at(&pool, &other, "1", at(3)).await;
    let repository =
        PgTokenPriceRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);

    let mut seen = Vec::new();
    let mut cursor = None;
    loop {
        let page = repository
            .list_prices(&pk(1), at(2), cursor.take(), 2)
            .await
            .unwrap();
        seen.extend(page.items.into_iter().map(|p| (p.mint, p.fetched_at)));
        match page.next_cursor {
            Some(Cursor::TokenPrice(next)) => {
                assert_eq!(next.from, at(2));
                cursor = Some(*next);
            }
            Some(other) => panic!("{other:?}"),
            None => break,
        }
    }

    assert_eq!(
        seen,
        [5, 4, 3, 2].map(|minutes| (pk(1), at(minutes))).to_vec()
    );
}
