use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use solana_pubkey::Pubkey;
use yog_core::{
    Page, RepositoryResult,
    domain::{
        LiveRecord, Pool, PoolCatalog, PoolCursor, PoolEntry, PoolScope, ProtocolPools,
        TokenCursor, TokenEntry, TokenMetadata, TokenMetadataLookup,
    },
};

use super::{OnPool, Paired, PoolPairs};
use crate::application::EnrichedToken;
use crate::testing::{make_metadata, make_pool, pk};

/// A catalog that records each batch it is asked.
struct Catalog {
    pools: Vec<Pool>,
    asked: Mutex<Vec<Vec<Pubkey>>>,
}

#[async_trait]
impl PoolCatalog for Catalog {
    async fn counts_by_protocol(&self) -> RepositoryResult<Vec<ProtocolPools>> {
        unreachable!()
    }
    async fn find_by_addresses(&self, addresses: &[Pubkey]) -> RepositoryResult<Vec<Pool>> {
        self.asked.lock().unwrap().push(addresses.to_vec());
        Ok(self
            .pools
            .iter()
            .filter(|p| addresses.contains(&p.pool_address))
            .cloned()
            .collect())
    }
    async fn list_entries(
        &self,
        _: PoolScope,
        _: Option<PoolCursor>,
        _: i64,
    ) -> RepositoryResult<Page<PoolEntry>> {
        unreachable!()
    }
    async fn find_entry(&self, _: &Pubkey) -> RepositoryResult<Option<PoolEntry>> {
        unreachable!()
    }
}

/// Metadata that records each batch it is asked, and refuses the one-by-one read.
struct Metadata {
    known: Vec<TokenMetadata>,
    asked: Mutex<Vec<Vec<Pubkey>>>,
}

#[async_trait]
impl TokenMetadataLookup for Metadata {
    async fn find_by_mint(&self, _: &Pubkey) -> RepositoryResult<Option<TokenMetadata>> {
        unreachable!("a pair reads its metadata by batch")
    }
    async fn find_by_mints(&self, mints: &[Pubkey]) -> RepositoryResult<Vec<TokenMetadata>> {
        self.asked.lock().unwrap().push(mints.to_vec());
        Ok(self
            .known
            .iter()
            .filter(|m| mints.contains(&m.mint))
            .cloned()
            .collect())
    }
    async fn list_entries(
        &self,
        _: Option<TokenCursor>,
        _: i64,
    ) -> RepositoryResult<Page<TokenEntry>> {
        unreachable!()
    }
}

/// An item on `pool`, carrying `mints` when it creates the pool.
struct Item {
    pool: Pubkey,
    mints: Option<(Pubkey, Pubkey)>,
}

impl OnPool for Item {
    fn pool_address(&self) -> Pubkey {
        self.pool
    }
    fn own_mints(&self) -> Option<(Pubkey, Pubkey)> {
        self.mints
    }
}

fn on(pool: Pubkey) -> Item {
    Item { pool, mints: None }
}

struct Fixture {
    pairs: PoolPairs,
    catalog: Arc<Catalog>,
    metadata: Arc<Metadata>,
}

fn fixture(pools: Vec<Pool>, known: Vec<TokenMetadata>) -> Fixture {
    let catalog = Arc::new(Catalog {
        pools,
        asked: Mutex::new(Vec::new()),
    });
    let metadata = Arc::new(Metadata {
        known,
        asked: Mutex::new(Vec::new()),
    });
    Fixture {
        pairs: PoolPairs::new(catalog.clone(), metadata.clone()),
        catalog,
        metadata,
    }
}

fn symbol_of(token: &EnrichedToken) -> Option<&str> {
    token.metadata.as_ref().and_then(|m| m.symbol.as_deref())
}

fn symbol(paired: &Paired<Item>) -> (Option<&str>, Option<&str>) {
    (symbol_of(&paired.token_a), symbol_of(&paired.token_b))
}

/// Mutation: a read per item, or per pool — the batch asks each pool and each mint once.
#[tokio::test]
async fn a_batch_reads_its_pools_and_mints_once_each() {
    let f = fixture(
        vec![
            make_pool(pk(1), pk(10), pk(11)),
            make_pool(pk(2), pk(10), pk(12)),
        ],
        vec![
            make_metadata(pk(10), "SOL"),
            make_metadata(pk(11), "MET"),
            make_metadata(pk(12), "USDC"),
        ],
    );

    let pairs = f
        .pairs
        .of(&[on(pk(1)), on(pk(2)), on(pk(1))])
        .await
        .unwrap();

    assert_eq!(pairs.len(), 2);
    assert_eq!(*f.catalog.asked.lock().unwrap(), vec![vec![pk(1), pk(2)]]);
    let asked = f.metadata.asked.lock().unwrap();
    assert_eq!(asked.len(), 1);
    let mut mints = asked[0].clone();
    mints.sort_unstable();
    let mut expected = vec![pk(10), pk(11), pk(12)];
    expected.sort_unstable();
    assert_eq!(mints, expected);
    let first = Paired::of(on(pk(1)), &pairs);
    assert_eq!(symbol(&first), (Some("SOL"), Some("MET")));
    let second = Paired::of(on(pk(2)), &pairs);
    assert_eq!(symbol(&second), (Some("SOL"), Some("USDC")));
}

/// Mutation: the item's own mints ignored — a pool just created, not yet in the catalog,
/// would come unpaired.
#[tokio::test]
async fn a_creation_is_paired_on_its_own_mints_without_the_catalog() {
    let f = fixture(Vec::new(), vec![make_metadata(pk(10), "SOL")]);
    let creation = Item {
        pool: pk(1),
        mints: Some((pk(10), pk(11))),
    };

    let pairs = f.pairs.of(&[creation]).await.unwrap();

    assert!(f.catalog.asked.lock().unwrap().is_empty());
    let paired = Paired::of(on(pk(1)), &pairs);
    assert_eq!(paired.token_a.mint, Some(pk(10)));
    assert_eq!(paired.token_b.mint, Some(pk(11)));
    assert_eq!(symbol(&paired), (Some("SOL"), None));
}

/// A pool the catalog omits comes with two unresolved sides, never an error; an empty batch
/// reads nothing.
#[tokio::test]
async fn an_unknown_pool_comes_unresolved_and_an_empty_batch_reads_nothing() {
    let f = fixture(Vec::new(), Vec::new());

    let pairs = f.pairs.of(&[on(pk(1))]).await.unwrap();
    let paired = Paired::of(on(pk(1)), &pairs);
    assert_eq!((paired.token_a.mint, paired.token_b.mint), (None, None));
    assert!(f.metadata.asked.lock().unwrap().is_empty());

    assert!(f.pairs.of::<Item>(&[]).await.unwrap().is_empty());
    assert_eq!(f.catalog.asked.lock().unwrap().len(), 1);
}

/// Mutation: the page's navigation dropped or its order changed while pairing its items.
#[tokio::test]
async fn a_page_keeps_its_navigation_and_order_and_pairs_each_item() {
    let f = fixture(
        vec![make_pool(pk(1), pk(10), pk(11))],
        vec![make_metadata(pk(10), "SOL")],
    );
    let cursor = yog_core::Cursor::Protocol(yog_core::domain::Protocol::MeteoraDammV2);
    let page = Page {
        items: vec![on(pk(2)), on(pk(1))],
        next_cursor: Some(cursor.clone()),
        prev_cursor: None,
        is_first: true,
        is_last: false,
    };

    let paired = f.pairs.page(page).await.unwrap();

    assert_eq!(paired.next_cursor, Some(cursor));
    assert_eq!(paired.prev_cursor, None);
    assert!(paired.is_first && !paired.is_last);
    let pools: Vec<Pubkey> = paired.items.iter().map(|p| p.item.pool).collect();
    assert_eq!(pools, [pk(2), pk(1)]);
    assert_eq!(paired.items[0].token_a.mint, None);
    assert_eq!(symbol(&paired.items[1]), (Some("SOL"), None));
}

/// Mutation: a creation's own mints dropped, as an event or as a live record, or A and B
/// exchanged — every new pool comes unpaired until yog-context resolves it, or swapped.
#[tokio::test]
async fn a_pool_creation_carries_its_own_pair_in_order() {
    use chrono::Utc;
    use solana_signature::Signature;
    use yog_core::domain::{MeteoraDammV2InitializePoolEvent, PoolCreationEvent};

    let f = fixture(Vec::new(), Vec::new());
    let creation = PoolCreationEvent::MeteoraDammV2(MeteoraDammV2InitializePoolEvent {
        pool_address: pk(1),
        signature: Signature::from([1; 64]),
        timestamp: Utc::now(),
        slot: 9,
        transaction_index: None,
        event_index: 0,
        token_a_mint: pk(10),
        token_b_mint: pk(11),
        creator: pk(2),
        payer: pk(3),
        alpha_vault: pk(4),
        sqrt_min_price: 1,
        sqrt_max_price: 2,
        sqrt_price: 1,
        liquidity: 3,
        activation_type: 0,
        activation_point: 0,
        collect_fee_mode: 0,
        pool_type: 0,
        token_a_flag: 0,
        token_b_flag: 0,
        token_a_amount: 0,
        token_b_amount: 0,
        total_amount_a: 0,
        total_amount_b: 0,
        pool_fees_raw: Vec::new(),
    });

    let live = [LiveRecord {
        id: 7,
        event: creation.clone(),
    }];

    let pairs = f.pairs.of(std::slice::from_ref(&creation)).await.unwrap();
    let live_pairs = f.pairs.of(&live).await.unwrap();

    for pairs in [pairs, live_pairs] {
        let paired = Paired::of(creation.clone(), &pairs);
        assert_eq!(paired.token_a.mint, Some(pk(10)));
        assert_eq!(paired.token_b.mint, Some(pk(11)));
    }
    assert!(f.catalog.asked.lock().unwrap().is_empty());
}
