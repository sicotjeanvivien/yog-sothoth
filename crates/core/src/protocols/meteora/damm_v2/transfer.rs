use crate::{CoreError, CoreResult};
use solana_pubkey::Pubkey;
use solana_transaction_status::{
    option_serializer::OptionSerializer, EncodedConfirmedTransactionWithStatusMeta,
    EncodedTransaction, UiInnerInstructions, UiInstruction, UiMessage, UiParsedInstruction,
    UiTransactionStatusMeta,
};
use std::str::FromStr;

/// Intermediate struct for a token transfer extracted from inner instructions.
pub(crate) struct TokenTransfer {
    pub(crate) mint: Pubkey,
    pub(crate) amount: u64,
    pub(crate) source: Pubkey,
    pub(crate) destination: Pubkey,
}

/// Find the two transferChecked instructions emitted as CPI by the DAMM v2 swap.
///
/// Locates the outer DAMM v2 swap instruction, then extracts the two token
/// transfers from its inner instruction group.
pub(super) fn extract_swap_transfers(
    tx: &EncodedConfirmedTransactionWithStatusMeta,
    meta: &UiTransactionStatusMeta,
    signature: &str,
    program_id_str: &str,
) -> CoreResult<(TokenTransfer, TokenTransfer, String, String)> {
    let candidates = find_damm_inner_group(tx, meta, signature, program_id_str)?;
    let transfers: Vec<&UiInstruction> = candidates
        .iter()
        .copied()
        .filter(|ix| is_transfer_checked(ix))
        .take(2)
        .collect();

    if transfers.len() < 2 {
        return Err(CoreError::ParseError {
            signature: signature.to_string(),
            reason: format!(
                "expected 2 transferChecked in DAMM v2 swap inner group, got {}",
                transfers.len()
            ),
        });
    }

    let transfer_in = extract_transfer(transfers[0], signature)?;
    let transfer_out = extract_transfer(transfers[1], signature)?;

    // vault_a = destination of transfer_in (user → pool)
    // vault_b = source of transfer_out (pool → user)
    let vault_a = transfer_in.destination.to_string();
    let vault_b = transfer_out.source.to_string();

    Ok((transfer_in, transfer_out, vault_a, vault_b))
}

/// Find the two transferChecked instructions for an AddLiquidity/RemoveLiquidity.
///
/// Locates the outer DAMM v2 instruction, then extracts the two token
/// transfers from its inner instruction group.
pub(super) fn extract_liquidity_transfers(
    tx: &EncodedConfirmedTransactionWithStatusMeta,
    meta: &UiTransactionStatusMeta,
    signature: &str,
    program_id_str: &str,
) -> CoreResult<(TokenTransfer, TokenTransfer)> {
    let candidates = find_damm_inner_group(tx, meta, signature, program_id_str)?;

    let transfers: Vec<&UiInstruction> = candidates
        .iter()
        .copied()
        .filter(|ix| is_transfer_checked(ix))
        .take(2)
        .collect();

    if transfers.len() < 2 {
        return Err(CoreError::ParseError {
            signature: signature.to_string(),
            reason: format!(
                "expected 2 transferChecked in DAMM v2 liquidity inner group, got {}",
                transfers.len()
            ),
        });
    }

    let transfer_a = extract_transfer(transfers[0], signature)?;
    let transfer_b = extract_transfer(transfers[1], signature)?;

    Ok((transfer_a, transfer_b))
}

// ============================================================
// Helpers
// ============================================================

/// Locate the inner instruction group attached to the first outer
/// instruction that invokes the DAMM v2 program.
fn find_damm_inner_group<'a>(
    tx: &'a EncodedConfirmedTransactionWithStatusMeta,
    meta: &'a UiTransactionStatusMeta,
    signature: &str,
    program_id_str: &str,
) -> CoreResult<Vec<&'a UiInstruction>> {
    let outer = outer_instructions(tx, signature)?;

    // Case 1 — DAMM v2 is an outer instruction
    if let Some(outer_idx) = outer
        .iter()
        .position(|ix| is_program_ix(ix, program_id_str))
    {
        let inner = match &meta.inner_instructions {
            OptionSerializer::Some(inner) => inner,
            _ => {
                return Err(CoreError::MissingField {
                    signature: signature.to_string(),
                    field: "innerInstructions".to_string(),
                })
            }
        };

        let group = inner
            .iter()
            .find(|g| g.index as usize == outer_idx)
            .ok_or_else(|| CoreError::ParseError {
                signature: signature.to_string(),
                reason: format!("no inner group for DAMM v2 outer ix at {outer_idx}"),
            })?;

        return Ok(group.instructions.iter().collect());
    }

    // Case 2 — DAMM v2 is an inner instruction (aggregator / router)
    if let OptionSerializer::Some(inner_groups) = &meta.inner_instructions {
        for group in inner_groups {
            if let Some(damm_idx) = group
                .instructions
                .iter()
                .position(|ix| is_program_ix(ix, program_id_str))
            {
                // Take all instructions after the DAMM v2 call within the same group
                return Ok(group.instructions.iter().skip(damm_idx + 1).collect());
            }
        }
    }

    Err(CoreError::ParseError {
        signature: signature.to_string(),
        reason: "no DAMM v2 instruction found in transaction".to_string(),
    })
}

/// Extract the outer instructions list from a parsed transaction.
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

/// Check if an instruction invokes a given program.
fn is_program_ix(ix: &UiInstruction, program_id: &str) -> bool {
    matches!(
        ix,
        UiInstruction::Parsed(UiParsedInstruction::PartiallyDecoded(p))
            if p.program_id == program_id
    )
}

/// Check if an instruction is a parsed transferChecked.
fn is_transfer_checked(ix: &UiInstruction) -> bool {
    matches!(
        ix,
        UiInstruction::Parsed(UiParsedInstruction::Parsed(p))
            if p.parsed.get("type").and_then(|t| t.as_str()) == Some("transferChecked")
    )
}

/// Extract mint, amount, source, and destination from a transferChecked instruction.
fn extract_transfer(ix: &UiInstruction, signature: &str) -> CoreResult<TokenTransfer> {
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
        })
        .and_then(|s| {
            Pubkey::from_str(s).map_err(|_| CoreError::ParseError {
                signature: signature.to_string(),
                reason: format!("invalid pubkey for mint: {s}"),
            })
        })?;

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

    let destination = info
        .get("destination")
        .and_then(|d| d.as_str())
        .ok_or_else(|| CoreError::MissingField {
            signature: signature.to_string(),
            field: "transferChecked.destination".to_string(),
        })
        .and_then(|s| {
            Pubkey::from_str(s).map_err(|_| CoreError::ParseError {
                signature: signature.to_string(),
                reason: format!("invalid pubkey for destination: {s}"),
            })
        })?;

    let source = info
        .get("source")
        .and_then(|d| d.as_str())
        .ok_or_else(|| CoreError::MissingField {
            signature: signature.to_string(),
            field: "transferChecked.source".to_string(),
        })
        .and_then(|s| {
            Pubkey::from_str(s).map_err(|_| CoreError::ParseError {
                signature: signature.to_string(),
                reason: format!("invalid pubkey for source: {s}"),
            })
        })?;

    Ok(TokenTransfer {
        mint,
        amount,
        source,
        destination,
    })
}
