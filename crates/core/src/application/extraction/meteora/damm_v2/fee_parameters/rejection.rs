use thiserror::Error;

/// Why a fee blob was not decoded: each reason its own metric label.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MeteoraDammV2FeeParametersRejection {
    #[error("unknown base fee mode {mode}")]
    UnknownBaseFeeMode { mode: u8 },
    #[error("`{field}`: Option tag {tag}, neither 0 nor 1")]
    InvalidOptionTag { field: &'static str, tag: u8 },
    #[error("`{field}` truncated: {len} bytes left, {need} needed")]
    Truncated {
        field: &'static str,
        len: usize,
        need: usize,
    },
    #[error("{len} bytes past the last field")]
    TrailingBytes { len: usize },
    /// A layout that moved past the base fee would land other bytes here.
    #[error("padding byte {value}, not 0")]
    NonZeroPadding { value: u8 },
}

impl MeteoraDammV2FeeParametersRejection {
    /// The metric label of the reason.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UnknownBaseFeeMode { .. } => "unknown_base_fee_mode",
            Self::InvalidOptionTag { .. } => "invalid_option_tag",
            Self::Truncated { .. } => "truncated",
            Self::TrailingBytes { .. } => "trailing_bytes",
            Self::NonZeroPadding { .. } => "non_zero_padding",
        }
    }
}
