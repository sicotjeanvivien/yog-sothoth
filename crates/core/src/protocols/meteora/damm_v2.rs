pub(super) mod detector;
pub(super) mod parser;
pub(super) mod reserves;
pub(super) mod transfer;

use crate::protocols::PoolIndexer;
use crate::{
    domain::{LiquidityEvent, SwapEvent},
    CoreResult,
};
use solana_pubkey::Pubkey;
use solana_pubkey::{self, pubkey};
use solana_transaction_status::EncodedConfirmedTransactionWithStatusMeta;

/// Meteora DAMM v2 program ID.
pub(crate) const DAMM_V2_PROGRAM_ID: Pubkey =
    pubkey!("cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG");

/// Meteora DAMM v2 protocol handler (x·y=k + dynamic fees + NFT positions).
pub struct DammV2 {
    pub pool_address: Pubkey,
    program_id_str: String,
}

impl DammV2 {
    pub fn new(pool_address: Pubkey) -> Self {
        Self {
            pool_address,
            program_id_str: DAMM_V2_PROGRAM_ID.to_string(),
        }
    }
}

impl PoolIndexer for DammV2 {
    fn program_id(&self) -> Pubkey {
        DAMM_V2_PROGRAM_ID
    }

    fn is_swap(&self, tx: &EncodedConfirmedTransactionWithStatusMeta) -> bool {
        detector::is_swap(tx, self.program_id_str.as_str())
    }

    fn is_add_liquidity(&self, _tx: &EncodedConfirmedTransactionWithStatusMeta) -> bool {
        // Phase 1 — to be implemented
        false
    }

    fn is_remove_liquidity(&self, _tx: &EncodedConfirmedTransactionWithStatusMeta) -> bool {
        // Phase 1 — to be implemented
        false
    }

    fn parse_swap(&self, tx: &EncodedConfirmedTransactionWithStatusMeta) -> CoreResult<SwapEvent> {
        parser::parse_swap(tx, self.pool_address, self.program_id_str.as_str())
    }

    fn parse_add_liquidity(
        &self,
        _tx: &EncodedConfirmedTransactionWithStatusMeta,
    ) -> CoreResult<LiquidityEvent> {
        // Phase 1 — to be implemented
        todo!("Phase 1 — to be implemented")
    }

    fn parse_remove_liquidity(
        &self,
        _tx: &EncodedConfirmedTransactionWithStatusMeta,
    ) -> CoreResult<LiquidityEvent> {
        // Phase 1 — to be implemented
        todo!("Phase 1 — to be implemented")
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use crate::CoreError;

    use super::*;
    use serde_json;
    use solana_pubkey;

    /// Load a real transaction JSON captured from the RPC.
    fn load_tx(json: &str) -> EncodedConfirmedTransactionWithStatusMeta {
        serde_json::from_str(json).expect("failed to deserialize transaction")
    }

    const SUCCESSFUL_SWAP_TX: &str = include_str!("../../../tests/fixtures/damm_v2_swap_ok.json");
    const FAILED_TX: &str = include_str!("../../../tests/fixtures/damm_v2_swap_failed.json");
    const MALFORMED_SWAP_TX: &str =
        include_str!("../../../tests/fixtures/damm_v2_swap_malformed.json");

    #[test]
    fn test_is_swap_returns_true_for_successful_swap() {
        let tx = load_tx(SUCCESSFUL_SWAP_TX);
        let pool = DammV2::new(pubkey!("CGPxT5d1uf9a8cKVJuZaJAU76t2EfLGbTmRbfvLLZp5j"));
        assert!(pool.is_swap(&tx));
    }

    #[test]
    fn test_is_swap_returns_false_for_failed_transaction() {
        let tx = load_tx(FAILED_TX);
        let pool = DammV2::new(pubkey!("CGPxT5d1uf9a8cKVJuZaJAU76t2EfLGbTmRbfvLLZp5j"));
        assert!(!pool.is_swap(&tx));
    }

    #[test]
    fn test_parse_swap_extracts_correct_amounts() {
        let tx = load_tx(SUCCESSFUL_SWAP_TX);
        let pool = DammV2::new(pubkey!("CGPxT5d1uf9a8cKVJuZaJAU76t2EfLGbTmRbfvLLZp5j"));
        let result = pool.parse_swap(&tx).unwrap();

        // From the captured transaction:
        // transferChecked #1: 133661157 SOL → vault
        // transferChecked #2: 10994840 USDC ← vault
        assert_eq!(result.amount_in, 133661157);
        assert_eq!(result.amount_out, 10994840);
        assert_eq!(
            result.token_in_mint,
            pubkey!("So11111111111111111111111111111111111111112")
        );
        assert_eq!(
            result.token_out_mint,
            pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")
        );
    }

    #[test]
    fn test_parse_swap_returns_err_for_malformed_transaction() {
        let tx = load_tx(MALFORMED_SWAP_TX);
        let pool = DammV2::new(pubkey!("CGPxT5d1uf9a8cKVJuZaJAU76t2EfLGbTmRbfvLLZp5j"));
        assert!(pool.is_swap(&tx));
        assert!(pool.parse_swap(&tx).is_err());
    }

    #[test]
    fn test_parse_swap_extracts_correct_reserves() {
        let tx = load_tx(SUCCESSFUL_SWAP_TX);
        let pool = DammV2::new(pubkey!("CGPxT5d1uf9a8cKVJuZaJAU76t2EfLGbTmRbfvLLZp5j"));
        let result = pool.parse_swap(&tx).unwrap();

        // From preTokenBalances — vault SOL (E3r3rs6C9bZbokaPiMEwmvPUtcd6CE2nuK8RSMQdE64E)
        // owner: HLnpSz9h2S4hiLQ43rnSD9XkcUThA7B8hQMKmDaiTLcC
        // pre:  85167550281
        // post: 85301211438
        assert_eq!(result.reserve_a_before, 85167550281);
        assert_eq!(result.reserve_a_after, 85301211438);

        // From preTokenBalances — vault USDC (HK2HggD4Eg1tAyr3gnRvNG32Z8v7s1NQGjH77b14qvsx)
        // owner: HLnpSz9h2S4hiLQ43rnSD9XkcUThA7B8hQMKmDaiTLcC
        // pre:  3178914121
        // post: 3167919281
        assert_eq!(result.reserve_b_before, 3178914121);
        assert_eq!(result.reserve_b_after, 3167919281);
    }
}
