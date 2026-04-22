use solana_rpc_client_api::response::transaction::Signature;
use yog_core::domain::Protocol;

pub(crate) struct QualifiedSignature {
    pub protocol: Protocol,
    pub signature: Signature,
}
