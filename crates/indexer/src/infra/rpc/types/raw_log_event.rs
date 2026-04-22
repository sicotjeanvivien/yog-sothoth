use solana_rpc_client_api::response::TransactionError;
use yog_core::domain::Protocol;

pub(crate) struct RawLogEvent {
    pub protocol: Protocol,
    pub signature: String,
    pub logs: Vec<String>,
    pub err: Option<TransactionError>,
}
