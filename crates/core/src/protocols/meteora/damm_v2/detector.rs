use solana_transaction_status::EncodedConfirmedTransactionWithStatusMeta;

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

    let solana_transaction_status::option_serializer::OptionSerializer::Some(logs) =
        &meta.log_messages
    else {
        return false;
    };

    let expected_log = format!("Program log: Instruction: {instruction_name}");

    logs.windows(2).any(|pair| {
        pair[0].contains(program_id_str) && pair[0].contains("invoke") && pair[1] == expected_log
    })
}
