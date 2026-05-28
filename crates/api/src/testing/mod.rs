//! Test support: fixtures and configurable repository mocks.
//!
//! Compiled only under `#[cfg(test)]`. Lives at the crate root so
//! every service test (PoolService today, others later) can share the
//! same mocks instead of redefining them per module.
//!
//! The mocks deliberately implement only what the services exercise;
//! unused trait methods are `unreachable!()`. Each "once" mock yields
//! its preset value a single time (via `Mutex<Option<...>>`) because
//! `RepositoryResult` is not necessarily `Clone`. A second call panics
//! loudly rather than returning a misleading value.

use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use solana_pubkey::Pubkey;
use std::collections::HashMap;
use std::sync::Mutex;

use yog_core::{
    Cursor, Page, PageDirection, PagePosition, RepositoryError, RepositoryResult,
    domain::{
        Pool, PoolAnalytics, PoolAnalyticsRepository, PoolCursor, PoolRepository, Protocol,
        TokenMetadata, TokenMetadataRepository, TokenPrice, TokenPriceRepository,
    },
};

// ── Fixtures ───────────────────────────────────────────────────────

/// Deterministic pubkey from a single byte, for readable tests.
pub(crate) fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

pub(crate) fn ts(secs: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(secs, 0).unwrap()
}

pub(crate) fn make_pool(addr: Pubkey, token_a: Pubkey, token_b: Pubkey) -> Pool {
    Pool {
        pool_address: addr,
        protocol: Protocol::MeteoraDammV2, // adapt to your variant name
        token_a_mint: token_a,
        token_b_mint: token_b,
        first_seen_at: ts(1_000),
        last_seen_at: ts(2_000),
    }
}

// NOTE: adapt the field set of TokenMetadata / TokenPrice to your real
// domain structs — only the Option-ness matters to the assertions.
pub(crate) fn make_metadata(mint: Pubkey, symbol: &str) -> TokenMetadata {
    TokenMetadata {
        mint,
        symbol: Some(symbol.to_string()),
        name: format!("{symbol} token").into(),
        decimals: 9,
        logo_uri: None,
        metadata_source: "Helius".to_string(),
        fetched_at: DateTime::from_timestamp_nanos(1662921288_000_000_000),
        last_refresh_at: DateTime::from_timestamp_nanos(1662921288_000_000_000),
    }
}

pub(crate) fn make_price(mint: Pubkey) -> TokenPrice {
    TokenPrice {
        mint,
        // adapt: rust_decimal::Decimal, source, fetched_at, etc.
        price_usd: rust_decimal::Decimal::new(100, 0),
        price_source: yog_core::domain::PriceSource::Helius,
        fetched_at: ts(1_500),
        confidence: Some(6.0),
    }
}

pub(crate) fn make_page(pools: Vec<Pool>, is_first: bool, is_last: bool) -> Page<Pool> {
    let prev = if is_first {
        None
    } else {
        pools.first().map(|p| {
            Cursor::Pool(PoolCursor {
                first_seen_at: p.first_seen_at,
                pool_address: p.pool_address,
            })
        })
    };
    let next = if is_last {
        None
    } else {
        pools.last().map(|p| {
            Cursor::Pool(PoolCursor {
                first_seen_at: p.first_seen_at,
                pool_address: p.pool_address,
            })
        })
    };
    Page {
        items: pools,
        next_cursor: next,
        prev_cursor: prev,
        is_first,
        is_last,
    }
}

fn take<T>(slot: &Mutex<Option<RepositoryResult<T>>>) -> RepositoryResult<T> {
    slot.lock()
        .unwrap()
        .take()
        .expect("mock method called more than once")
}

// ── Mock: PoolRepository ────────────────────────────────────────────

pub(crate) struct PoolRepoOnce {
    paginated: Mutex<Option<RepositoryResult<Page<Pool>>>>,
    by_address: Mutex<Option<RepositoryResult<Option<Pool>>>>,
}

impl PoolRepoOnce {
    pub(crate) fn with_page(page: Page<Pool>) -> Self {
        Self {
            paginated: Mutex::new(Some(Ok(page))),
            by_address: Mutex::new(Some(Ok(None))),
        }
    }
    pub(crate) fn with_pool(pool: Option<Pool>) -> Self {
        Self {
            paginated: Mutex::new(Some(Ok(make_page(vec![], true, true)))),
            by_address: Mutex::new(Some(Ok(pool))),
        }
    }
    pub(crate) fn with_paginate_err() -> Self {
        Self {
            paginated: Mutex::new(Some(Err(RepositoryError::Integrity("boom".into())))),
            by_address: Mutex::new(Some(Ok(None))),
        }
    }
}

#[async_trait]
impl PoolRepository for PoolRepoOnce {
    async fn upsert(&self, _pool: &Pool) -> RepositoryResult<()> {
        unreachable!("upsert not used by PoolService")
    }
    async fn touch_last_seen(&self, _addr: &Pubkey) -> RepositoryResult<()> {
        unreachable!("touch_last_seen not used by PoolService")
    }
    async fn find_by_address(&self, _addr: &Pubkey) -> RepositoryResult<Option<Pool>> {
        take(&self.by_address)
    }
    async fn find_paginated(
        &self,
        _cursor: Option<PoolCursor>,
        _direction: PageDirection,
        _position: Option<PagePosition>,
        _search: Option<String>,
        _limit: i64,
    ) -> RepositoryResult<Page<Pool>> {
        take(&self.paginated)
    }
}

// ── Mock: PoolAnalyticsRepository ───────────────────────────────────

pub(crate) struct MockAnalyticsRepo {
    map: HashMap<Pubkey, PoolAnalytics>,
    fail: bool,
}

impl MockAnalyticsRepo {
    pub(crate) fn with(map: HashMap<Pubkey, PoolAnalytics>) -> Self {
        Self { map, fail: false }
    }
    pub(crate) fn empty() -> Self {
        Self {
            map: HashMap::new(),
            fail: false,
        }
    }
    pub(crate) fn failing() -> Self {
        Self {
            map: HashMap::new(),
            fail: true,
        }
    }
}

#[async_trait]
impl PoolAnalyticsRepository for MockAnalyticsRepo {
    async fn batch_compute(
        &self,
        addresses: &[Pubkey],
    ) -> RepositoryResult<HashMap<Pubkey, PoolAnalytics>> {
        if self.fail {
            return Err(RepositoryError::Integrity("analytics boom".into()));
        }
        let mut out = HashMap::new();
        for a in addresses {
            if let Some(v) = self.map.get(a) {
                out.insert(*a, v.clone());
            }
        }
        Ok(out)
    }
}

// ── Mock: TokenMetadataRepository ───────────────────────────────────

pub(crate) struct MockMetadataRepo {
    by_mint: HashMap<Pubkey, TokenMetadata>,
}

impl MockMetadataRepo {
    pub(crate) fn with(entries: Vec<(Pubkey, TokenMetadata)>) -> Self {
        Self {
            by_mint: entries.into_iter().collect(),
        }
    }
    pub(crate) fn empty() -> Self {
        Self {
            by_mint: HashMap::new(),
        }
    }
}

#[async_trait]
impl TokenMetadataRepository for MockMetadataRepo {
    async fn upsert(&self, _m: &TokenMetadata) -> RepositoryResult<()> {
        unreachable!()
    }
    async fn list_known_mints(&self) -> RepositoryResult<Vec<Pubkey>> {
        unreachable!()
    }
    async fn list_missing_mints(&self) -> RepositoryResult<Vec<Pubkey>> {
        unreachable!()
    }
    async fn find_by_mint(&self, mint: &Pubkey) -> RepositoryResult<Option<TokenMetadata>> {
        Ok(self.by_mint.get(mint).cloned())
    }
}

// ── Mock: TokenPriceRepository ──────────────────────────────────────

pub(crate) struct MockPriceRepo {
    by_mint: HashMap<Pubkey, TokenPrice>,
}

impl MockPriceRepo {
    pub(crate) fn with(entries: Vec<(Pubkey, TokenPrice)>) -> Self {
        Self {
            by_mint: entries.into_iter().collect(),
        }
    }
    pub(crate) fn empty() -> Self {
        Self {
            by_mint: HashMap::new(),
        }
    }
}

#[async_trait]
impl TokenPriceRepository for MockPriceRepo {
    async fn insert_batch(&self, _p: &[TokenPrice]) -> RepositoryResult<()> {
        unreachable!()
    }
    async fn find_latest_by_mint(&self, mint: &Pubkey) -> RepositoryResult<Option<TokenPrice>> {
        Ok(self.by_mint.get(mint).cloned())
    }
}
