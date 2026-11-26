use solana_pubkey::Pubkey;
use thiserror::Error;

use crate::domain::Protocol;

/// Why an account was not decoded.
/// ⚠️ Not an `Option`: a pool that never resolves is re-fetched every cycle, so
/// each reason must reach its own log line and metric.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PoolAccountRejection {
    /// Owned by a program we do not index: a `pools` row points at the wrong account.
    #[error("account owned by an unindexed program: {program_id}")]
    UnknownProgram { program_id: Pubkey },

    /// Right program, wrong account: the Anchor discriminator does not match.
    #[error("{protocol}: not a pool account (unexpected discriminator)")]
    NotAPoolAccount { protocol: Protocol },

    /// Shorter than the layout requires: most likely an ABI change of the program.
    #[error("{protocol}: account too short — {len} bytes, layout needs {min}")]
    Truncated {
        protocol: Protocol,
        len: usize,
        min: usize,
    },
}
