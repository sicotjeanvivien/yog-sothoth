use crate::protocols::meteora::DammV2SwapResult;
use crate::{CoreError, CoreResult};
use chrono::{DateTime, Utc};
use solana_transaction_status::{
    EncodedConfirmedTransactionWithStatusMeta, EncodedTransaction, UiInstruction,
    UiParsedInstruction, UiTransactionStatusMeta,
};

/// Meteora DAMM v2 program ID.
const DAMM_V2_PROGRAM_ID: &str = "cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG";

/// Meteora DAMM v2 protocol handler (x·y=k + dynamic fees + NFT positions).
pub struct DammV2;

impl DammV2 {
    /// Check if a transaction contains a successful DAMM v2 swap.
    pub fn is_swap(tx: &EncodedConfirmedTransactionWithStatusMeta) -> bool {
        let Some(meta) = &tx.transaction.meta else {
            return false;
        };

        // Reject failed transactions
        if meta.err.is_some() {
            return false;
        }

        // Check log messages for DAMM v2 swap instruction
        let solana_transaction_status::option_serializer::OptionSerializer::Some(logs) =
            &meta.log_messages
        else {
            return false;
        };

        logs.windows(2).any(|pair| {
            pair[0].contains(DAMM_V2_PROGRAM_ID)
                && pair[0].contains("invoke")
                && pair[1] == "Program log: Instruction: Swap"
        })
    }

    pub fn is_not_swap(tx: &EncodedConfirmedTransactionWithStatusMeta) -> bool {
        !Self::is_swap(tx)
    }

    /// Parse a DAMM v2 swap from a confirmed transaction.
    pub fn parse_swap(
        tx: &EncodedConfirmedTransactionWithStatusMeta,
        pool_address: &str,
    ) -> CoreResult<Option<DammV2SwapResult>> {
        if Self::is_not_swap(tx) {
            return Ok(None);
        }

        let meta = tx
            .transaction
            .meta
            .as_ref()
            .ok_or_else(|| CoreError::ParseError {
                signature: String::new(),
                reason: "missing transaction meta".to_string(),
            })?;

        let signature = extract_signature(tx)?;
        let timestamp = extract_timestamp(tx)?;

        // Find the two transferChecked instructions that follow the DAMM v2 swap
        let (transfer_in, transfer_out) = extract_swap_transfers(meta, &signature)?;

        // Extract reserves from pre/post token balances
        let (reserve_a_before, reserve_b_before, reserve_a_after, reserve_b_after) =
            extract_reserves(
                meta,
                &transfer_in.vault_address,
                &transfer_out.vault_address,
                &signature,
            )?;

        Ok(Some(DammV2SwapResult {
            pool_address: pool_address.to_string(),
            token_in_mint: transfer_in.mint,
            token_out_mint: transfer_out.mint,
            amount_in: transfer_in.amount,
            amount_out: transfer_out.amount,
            reserve_a_before,
            reserve_b_before,
            reserve_a_after,
            reserve_b_after,
            signature,
            timestamp,
        }))
    }
}

/// Intermediate struct for a token transfer extracted from inner instructions.
struct TokenTransfer {
    mint: String,
    amount: u64,
    vault_address: String,
}

/// Extract the transaction signature.
fn extract_signature(tx: &EncodedConfirmedTransactionWithStatusMeta) -> CoreResult<String> {
    match &tx.transaction.transaction {
        EncodedTransaction::Json(ui_tx) => {
            ui_tx
                .signatures
                .first()
                .cloned()
                .ok_or_else(|| CoreError::MissingField {
                    signature: String::new(),
                    field: "signatures".to_string(),
                })
        }
        _ => Err(CoreError::ParseError {
            signature: String::new(),
            reason: "unexpected transaction encoding".to_string(),
        }),
    }
}

/// Extract the block timestamp.
fn extract_timestamp(tx: &EncodedConfirmedTransactionWithStatusMeta) -> CoreResult<DateTime<Utc>> {
    let block_time = tx.block_time.ok_or_else(|| CoreError::MissingField {
        signature: String::new(),
        field: "blockTime".to_string(),
    })?;

    DateTime::from_timestamp(block_time, 0).ok_or_else(|| CoreError::ParseError {
        signature: String::new(),
        reason: format!("invalid timestamp: {block_time}"),
    })
}

/// Find the two transferChecked instructions that immediately follow
/// the DAMM v2 swap inner instruction.
fn extract_swap_transfers(
    meta: &UiTransactionStatusMeta,
    signature: &str,
) -> CoreResult<(TokenTransfer, TokenTransfer)> {
    use solana_transaction_status::option_serializer::OptionSerializer;

    let inner_instructions = match &meta.inner_instructions {
        OptionSerializer::Some(inner) => inner,
        _ => {
            return Err(CoreError::MissingField {
                signature: signature.to_string(),
                field: "innerInstructions".to_string(),
            })
        }
    };

    // Find the instruction group that contains the DAMM v2 swap
    for group in inner_instructions {
        let instructions = &group.instructions;

        // Find the DAMM v2 swap instruction index
        let damm_swap_idx = instructions.iter().position(|ix| {
            matches!(ix, UiInstruction::Compiled(c) if {
                // For compiled instructions we check accounts involved
                // The DAMM v2 instruction data starts with a known discriminator
                false // handled below via parsed check
            }) || matches!(ix, UiInstruction::Parsed(UiParsedInstruction::PartiallyDecoded(p))
                if p.program_id == DAMM_V2_PROGRAM_ID && p.data.starts_with("PgQWtn8"))
        });

        if let Some(idx) = damm_swap_idx {
            // The two transferChecked follow immediately after
            let transfers: Vec<&UiInstruction> = instructions
                .iter()
                .skip(idx + 1)
                .filter(|ix| is_transfer_checked(ix))
                .take(2)
                .collect();

            if transfers.len() < 2 {
                return Err(CoreError::ParseError {
                    signature: signature.to_string(),
                    reason: "expected 2 transferChecked after DAMM v2 swap".to_string(),
                });
            }

            let transfer_in = parse_transfer_checked(transfers[0], signature)?;
            let transfer_out = parse_transfer_checked(transfers[1], signature)?;

            return Ok((transfer_in, transfer_out));
        }
    }

    Err(CoreError::ParseError {
        signature: signature.to_string(),
        reason: "no DAMM v2 swap instruction found in inner instructions".to_string(),
    })
}

/// Check if an instruction is a parsed transferChecked.
fn is_transfer_checked(ix: &UiInstruction) -> bool {
    matches!(ix, UiInstruction::Parsed(UiParsedInstruction::Parsed(p))
        if p.parsed.get("type").and_then(|t| t.as_str()) == Some("transferChecked"))
}

/// Extract mint, amount, and vault address from a transferChecked instruction.
fn parse_transfer_checked(ix: &UiInstruction, signature: &str) -> CoreResult<TokenTransfer> {
    let parsed = match ix {
        UiInstruction::Parsed(UiParsedInstruction::Parsed(p)) => &p.parsed,
        _ => {
            return Err(CoreError::ParseError {
                signature: signature.to_string(),
                reason: "expected parsed transferChecked instruction".to_string(),
            })
        }
    };

    let info = parsed.get("info").ok_or_else(|| CoreError::MissingField {
        signature: signature.to_string(),
        field: "transferChecked.info".to_string(),
    })?;

    let mint = info
        .get("mint")
        .and_then(|m| m.as_str())
        .ok_or_else(|| CoreError::MissingField {
            signature: signature.to_string(),
            field: "transferChecked.mint".to_string(),
        })?
        .to_string();

    let amount_str = info
        .get("tokenAmount")
        .and_then(|ta| ta.get("amount"))
        .and_then(|a| a.as_str())
        .ok_or_else(|| CoreError::MissingField {
            signature: signature.to_string(),
            field: "transferChecked.tokenAmount.amount".to_string(),
        })?;

    let amount = amount_str
        .parse::<u64>()
        .map_err(|_| CoreError::ParseError {
            signature: signature.to_string(),
            reason: format!("invalid token amount: {amount_str}"),
        })?;

    // vault_address = destination of transfer_in (user → pool vault)
    let vault_address = info
        .get("destination")
        .and_then(|d| d.as_str())
        .ok_or_else(|| CoreError::MissingField {
            signature: signature.to_string(),
            field: "transferChecked.destination".to_string(),
        })?
        .to_string();

    Ok(TokenTransfer {
        mint,
        amount,
        vault_address,
    })
}

/// Extract pre/post reserves for the two pool vaults.
fn extract_reserves(
    meta: &UiTransactionStatusMeta,
    vault_a_address: &str,
    vault_b_address: &str,
    signature: &str,
) -> CoreResult<(u64, u64, u64, u64)> {
    use solana_transaction_status::option_serializer::OptionSerializer;

    let _pre = match &meta.pre_token_balances {
        OptionSerializer::Some(b) => b,
        _ => {
            return Err(CoreError::MissingField {
                signature: signature.to_string(),
                field: "preTokenBalances".to_string(),
            })
        }
    };

    let _post = match &meta.post_token_balances {
        OptionSerializer::Some(b) => b,
        _ => {
            return Err(CoreError::MissingField {
                signature: signature.to_string(),
                field: "postTokenBalances".to_string(),
            })
        }
    };

    // TODO: match vaults by account index from the message account keys
    Ok((0, 0, 0, 0))
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
}
