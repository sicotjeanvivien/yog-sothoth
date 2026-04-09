pub(crate) mod detector;
pub(crate) mod parser;
pub(crate) mod reserves;
pub(crate) mod transfer;

use crate::types::DammV2SwapResult;
use crate::CoreResult;
use solana_transaction_status::EncodedConfirmedTransactionWithStatusMeta;

/// Meteora DAMM v2 program ID.
pub(crate) const DAMM_V2_PROGRAM_ID: &str = "cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG";

/// Meteora DAMM v2 protocol handler (x·y=k + dynamic fees + NFT positions).
pub struct DammV2;

impl DammV2 {
    pub fn is_swap(tx: &EncodedConfirmedTransactionWithStatusMeta) -> bool {
        detector::is_swap(tx)
    }

    pub fn parse_swap(
        tx: &EncodedConfirmedTransactionWithStatusMeta,
        pool_address: &str,
    ) -> CoreResult<Option<DammV2SwapResult>> {
        parser::parse_swap(tx, pool_address)
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    /// Load a real transaction JSON captured from the RPC.
    fn load_tx(json: &str) -> EncodedConfirmedTransactionWithStatusMeta {
        serde_json::from_str(json).expect("failed to deserialize transaction")
    }

    const SUCCESSFUL_SWAP_TX: &str = include_str!("../../../tests/fixtures/damm_v2_swap_ok.json");
    const FAILED_TX: &str = include_str!("../../../tests/fixtures/damm_v2_swap_failed.json");

    #[test]
    fn test_is_swap_returns_true_for_successful_swap() {
        let tx = load_tx(SUCCESSFUL_SWAP_TX);
        assert!(DammV2::is_swap(&tx));
    }

    #[test]
    fn test_is_swap_returns_false_for_failed_transaction() {
        let tx = load_tx(FAILED_TX);
        assert!(!DammV2::is_swap(&tx));
    }

    #[test]
    fn test_parse_swap_extracts_correct_amounts() {
        let tx = load_tx(SUCCESSFUL_SWAP_TX);
        let result = DammV2::parse_swap(&tx, "CGPxT5d1uf9a8cKVJuZaJAU76t2EfLGbTmRbfvLLZp5j")
            .expect("parse_swap failed")
            .expect("expected Some(result)");

        // From the captured transaction:
        // transferChecked #1: 133661157 SOL → vault
        // transferChecked #2: 10994840 USDC ← vault
        assert_eq!(result.amount_in, 133661157);
        assert_eq!(result.amount_out, 10994840);
        assert_eq!(
            result.token_in_mint,
            "So11111111111111111111111111111111111111112"
        );
        assert_eq!(
            result.token_out_mint,
            "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
        );
    }

    #[test]
    fn test_parse_swap_returns_none_for_failed_transaction() {
        let tx = load_tx(FAILED_TX);
        let result = DammV2::parse_swap(&tx, "CGPxT5d1uf9a8cKVJuZaJAU76t2EfLGbTmRbfvLLZp5j")
            .expect("parse_swap failed");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_swap_extracts_correct_reserves() {
        let tx = load_tx(SUCCESSFUL_SWAP_TX);
        let result = DammV2::parse_swap(&tx, "CGPxT5d1uf9a8cKVJuZaJAU76t2EfLGbTmRbfvLLZp5j")
            .expect("parse_swap failed")
            .expect("expected Some(result)");

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
