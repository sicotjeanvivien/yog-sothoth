use crate::{CoreError, CoreResult};
use solana_pubkey::Pubkey;
use solana_transaction_status::{
    option_serializer::OptionSerializer, EncodedConfirmedTransactionWithStatusMeta,
    EncodedTransaction, UiInstruction, UiMessage, UiParsedInstruction,
    UiPartiallyDecodedInstruction,
};
use std::str::FromStr;

/// Position of the pool account in a DAMM v2 swap/liquidity instruction.
/// Defined by the Meteora DAMM v2 IDL (cp-amm program).
const POOL_ACCOUNT_INDEX: usize = 0;

/// Extract the pool address from a DAMM v2 transaction.
///
/// Looks first in the outer instructions, then falls back to inner
/// instructions — DAMM v2 may be invoked directly by the user or via
/// an aggregator/router that wraps it in a CPI.
pub(super) fn extract_pool_address(
    tx: &EncodedConfirmedTransactionWithStatusMeta,
    program_id_str: &str,
    signature: &str,
) -> CoreResult<Pubkey> {
    // 1. Try outer instructions first (direct DAMM v2 call)
    for ix in outer_instructions(tx, signature)? {
        if let Some(pool) = pool_from_instruction(ix, program_id_str, signature)? {
            return Ok(pool);
        }
    }

    // 2. Fall back to inner instructions (aggregator / router case)
    if let Some(meta) = tx.transaction.meta.as_ref() {
        if let OptionSerializer::Some(inner_groups) = &meta.inner_instructions {
            for group in inner_groups {
                for ix in &group.instructions {
                    if let Some(pool) = pool_from_instruction(ix, program_id_str, signature)? {
                        return Ok(pool);
                    }
                }
            }
        }
    }

    Err(CoreError::ParseError {
        signature: signature.to_string(),
        reason: "no DAMM v2 instruction found in transaction".to_string(),
    })
}

/// If this instruction invokes DAMM v2, extract and return the pool pubkey.
/// Returns `Ok(None)` if the instruction is not DAMM v2.
fn pool_from_instruction(
    ix: &UiInstruction,
    program_id_str: &str,
    signature: &str,
) -> CoreResult<Option<Pubkey>> {
    let UiInstruction::Parsed(UiParsedInstruction::PartiallyDecoded(
        UiPartiallyDecodedInstruction { program_id, accounts, .. },
    )) = ix
    else {
        return Ok(None);
    };

    if program_id != program_id_str {
        return Ok(None);
    }

    let pool_str = accounts.get(POOL_ACCOUNT_INDEX).ok_or_else(|| {
        CoreError::ParseError {
            signature: signature.to_string(),
            reason: format!(
                "DAMM v2 instruction has fewer than {} accounts",
                POOL_ACCOUNT_INDEX + 1
            ),
        }
    })?;

    let pool = Pubkey::from_str(pool_str).map_err(|e| CoreError::ParseError {
        signature: signature.to_string(),
        reason: format!("invalid pool pubkey: {e}"),
    })?;

    Ok(Some(pool))
}

fn outer_instructions<'a>(
    tx: &'a EncodedConfirmedTransactionWithStatusMeta,
    signature: &str,
) -> CoreResult<&'a Vec<UiInstruction>> {
    match &tx.transaction.transaction {
        EncodedTransaction::Json(ui_tx) => match &ui_tx.message {
            UiMessage::Parsed(parsed) => Ok(&parsed.instructions),
            UiMessage::Raw(_) => Err(CoreError::ParseError {
                signature: signature.to_string(),
                reason: "expected JsonParsed encoding, got Raw".to_string(),
            }),
        },
        _ => Err(CoreError::ParseError {
            signature: signature.to_string(),
            reason: "unexpected transaction encoding".to_string(),
        }),
    }
}