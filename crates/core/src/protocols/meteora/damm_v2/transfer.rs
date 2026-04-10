use solana_transaction_status::{UiInstruction, UiParsedInstruction, UiTransactionStatusMeta};
use crate::{CoreError, CoreResult};
use std::str::FromStr;
use solana_pubkey::Pubkey;

/// Intermediate struct for a token transfer extracted from inner instructions.
pub(crate) struct TokenTransfer {
    pub(crate) mint: Pubkey,
    pub(crate) amount: u64,
    pub(crate) source: Pubkey,
    pub(crate) destination: Pubkey,
}

/// Find the two transferChecked instructions that immediately follow
/// the DAMM v2 swap inner instruction.
pub(super) fn extract_swap_transfers(
    meta: &UiTransactionStatusMeta,
    signature: &str,
    program_id_str: &str,
) -> CoreResult<(TokenTransfer, TokenTransfer, String, String)> {
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
            matches!(ix, UiInstruction::Parsed(UiParsedInstruction::PartiallyDecoded(p))
        if p.program_id == program_id_str)
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

            let transfer_in = extract_transfer(transfers[0], signature)?;
            let transfer_out = extract_transfer(transfers[1], signature)?;

            // vault_a = destination of transfer_in (user → pool)
            // vault_b = source of transfer_out (pool → user)
            let vault_a = transfer_in.destination.clone().to_string();
            let vault_b = transfer_out.source.clone().to_string();

            return Ok((transfer_in, transfer_out, vault_a, vault_b));
        }
    }

    Err(CoreError::ParseError {
        signature: signature.to_string(),
        reason: "no DAMM v2 swap instruction found in inner instructions".to_string(),
    })
}

/// Extract mint, amount, and vault address from a transferChecked instruction.
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
            field: "transferChecked.source".to_string(), // corrigé : était "destination"
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
        destination,
        source,
    })
}
/// Check if an instruction is a parsed transferChecked.
fn is_transfer_checked(ix: &UiInstruction) -> bool {
    matches!(ix, UiInstruction::Parsed(UiParsedInstruction::Parsed(p))
        if p.parsed.get("type").and_then(|t| t.as_str()) == Some("transferChecked"))
}
