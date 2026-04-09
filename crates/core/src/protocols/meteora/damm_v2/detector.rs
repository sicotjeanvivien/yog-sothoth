use super::DAMM_V2_PROGRAM_ID;
use solana_transaction_status::EncodedConfirmedTransactionWithStatusMeta;

pub(crate) fn is_swap(tx: &EncodedConfirmedTransactionWithStatusMeta) -> bool {
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
