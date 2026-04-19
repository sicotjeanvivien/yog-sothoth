use crate::{CoreError, CoreResult};
use solana_pubkey::Pubkey;
use solana_transaction_status::{
    EncodedConfirmedTransactionWithStatusMeta, EncodedTransaction, UiInstruction,
    UiMessage, UiParsedInstruction, UiPartiallyDecodedInstruction,
};
use std::str::FromStr;

/// Position of the pool account in a DAMM v2 swap/liquidity instruction.
/// Defined by the Meteora DAMM v2 IDL (cp-amm program).
const POOL_ACCOUNT_INDEX: usize = 1;

/// Extract the pool address from a DAMM v2 transaction.
///
/// Finds the outer instruction that invokes the DAMM v2 program and reads
/// the pool account at the IDL-defined position.
pub(super) fn extract_pool_address(
    tx: &EncodedConfirmedTransactionWithStatusMeta,
    program_id_str: &str,
    signature: &str,
) -> CoreResult<Pubkey> {
    let instructions = outer_instructions(tx, signature)?;

    for ix in instructions {
        if let Some(accounts) = accounts_if_program(ix, program_id_str) {
            let pool_str = accounts.get(POOL_ACCOUNT_INDEX).ok_or_else(|| {
                CoreError::ParseError {
                    signature: signature.to_string(),
                    reason: format!(
                        "DAMM v2 instruction has fewer than {} accounts",
                        POOL_ACCOUNT_INDEX + 1
                    ),
                }
            })?;

            return Pubkey::from_str(pool_str).map_err(|e| CoreError::ParseError {
                signature: signature.to_string(),
                reason: format!("invalid pool pubkey: {e}"),
            });
        }
    }

    Err(CoreError::ParseError {
        signature: signature.to_string(),
        reason: "no DAMM v2 instruction found in transaction".to_string(),
    })
}

/// Helper: extract the outer instructions list from a parsed transaction.
pub(super) fn outer_instructions<'a>(
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

/// Helper: if this instruction targets the given program, return its account list.
fn accounts_if_program<'a>(ix: &'a UiInstruction, program_id: &str) -> Option<&'a Vec<String>> {
    match ix {
        UiInstruction::Parsed(UiParsedInstruction::PartiallyDecoded(
            UiPartiallyDecodedInstruction { program_id: pid, accounts, .. },
        )) if pid == program_id => Some(accounts),
        _ => None,
    }
}