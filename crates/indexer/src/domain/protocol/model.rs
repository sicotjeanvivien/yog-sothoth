/// Supported AMM protocols.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Protocol {
    DammV2,
    DammV1,
    Dlmm,
}

impl Protocol {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Protocol::DammV2 => "damm_v2",
            Protocol::DammV1 => "damm_v1",
            Protocol::Dlmm => "dlmm",
        }
    }

    pub(crate) fn from_str(s: &str) -> Option<Self> {
        match s {
            "damm_v2" => Some(Protocol::DammV2),
            "damm_v1" => Some(Protocol::DammV1),
            "dlmm" => Some(Protocol::Dlmm),
            _ => None,
        }
    }
}
