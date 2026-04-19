use solana_transaction_status::{
    option_serializer::OptionSerializer, EncodedConfirmedTransactionWithStatusMeta,
};

pub(super) fn is_swap(
    tx: &EncodedConfirmedTransactionWithStatusMeta,
    program_id_str: &str,
) -> bool {
    is_instruction(tx, program_id_str, "Swap")
}

pub(super) fn is_add_liquidity(
    tx: &EncodedConfirmedTransactionWithStatusMeta,
    program_id_str: &str,
) -> bool {
    is_instruction(tx, program_id_str, "AddLiquidity")
}

pub(super) fn is_remove_liquidity(
    tx: &EncodedConfirmedTransactionWithStatusMeta,
    program_id_str: &str,
) -> bool {
    is_instruction(tx, program_id_str, "RemoveLiquidity")
}

fn is_instruction(
    tx: &EncodedConfirmedTransactionWithStatusMeta,
    program_id_str: &str,
    instruction_name: &str,
) -> bool {
    let Some(meta) = &tx.transaction.meta else {
        return false;
    };
    if meta.err.is_some() {
        return false;
    }
    let OptionSerializer::Some(logs) = &meta.log_messages else {
        return false;
    };
    let expected_log = format!("Program log: Instruction: {instruction_name}");
    let invoke_marker = format!("Program {program_id_str} invoke");

    let mut in_target_program = false;
    for log in logs {
        if log.starts_with(&invoke_marker) {
            in_target_program = true;
            continue;
        }
        if in_target_program {
            if log == &expected_log {
                return true;
            }
            // Tout ce qui n'est pas un log du programme cible ferme la fenêtre
            if log.starts_with("Program ") && !log.starts_with("Program log:") {
                in_target_program = false;
            }
        }
    }
    false
}
