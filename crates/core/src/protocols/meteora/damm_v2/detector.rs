use solana_transaction_status::EncodedConfirmedTransactionWithStatusMeta;

pub(super) fn is_swap(
    tx: &EncodedConfirmedTransactionWithStatusMeta,
    program_id_str: &str,
) -> bool {
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

    // TODO(perf): DAMM_V2_PROGRAM_ID.to_string() alloue à chaque appel — cacher via LazyLock<String> si hot path
    logs.windows(2).any(|pair| {
        pair[0].contains(program_id_str)
            && pair[0].contains("invoke")
            && pair[1] == "Program log: Instruction: Swap"
    })
}
