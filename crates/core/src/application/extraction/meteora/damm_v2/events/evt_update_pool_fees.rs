//! Wire mirror of `cp-amm::EvtUpdatePoolFees` and its Anchor discriminator.

use super::compute_discriminator;
use crate::application::extraction::DISCRIMINATOR_LEN;
use borsh::BorshDeserialize;
use solana_pubkey::Pubkey;

/// Discriminator for [`EvtUpdatePoolFees`].
pub fn discriminator_update_pool_fees() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtUpdatePoolFees")
}

/// Mirror of `cp-amm::EvtUpdatePoolFees`.
///
/// Emitted when a pool's fee parameters are updated by an operator. The
/// nested `UpdatePoolFeesParameters` is **not** modelled — there is no test
/// fixture to validate its (version-sensitive) layout, and "voie C" defers
/// fee interpretation anyway. Instead, [`BorshDeserialize`] reads the two
/// leading pubkeys and captures the remaining payload bytes verbatim into
/// `params_raw`. This is robust to fee-struct schema changes: a later decode
/// works from these stored bytes.
#[derive(Debug, Clone)]
pub struct EvtUpdatePoolFees {
    pub pool: Pubkey,
    pub operator: Pubkey,
    /// Raw, undecoded bytes of the trailing `UpdatePoolFeesParameters`.
    pub params_raw: Vec<u8>,
}

impl BorshDeserialize for EvtUpdatePoolFees {
    fn deserialize_reader<R: borsh::io::Read>(reader: &mut R) -> borsh::io::Result<Self> {
        let pool = Pubkey::deserialize_reader(reader)?;
        let operator = Pubkey::deserialize_reader(reader)?;
        let mut params_raw = Vec::new();
        reader.read_to_end(&mut params_raw)?;
        Ok(Self {
            pool,
            operator,
            params_raw,
        })
    }
}
