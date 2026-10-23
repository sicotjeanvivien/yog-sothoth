//! Which bank of a slot a message belongs to. Since Alpenglow a slot may carry several, and only
//! the slot status names the one the cluster kept.
//! ⚠️ A bank id is a counter of the serving node: it means something within one stream session only.

/// The bank a message names, or that it names none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Bank {
    Id(u64),
    /// A server before Yellowstone v16 sends none: its slot is paired by slot alone.
    Unknown,
}

impl Bank {
    /// From a transaction's or a block-meta's `bank_id`.
    /// ⚠️ 0 is "not sent", not a bank: the field is a plain `uint64`, so an absent one reads 0.
    pub(crate) fn from_wire(id: u64) -> Self {
        match id {
            0 => Self::Unknown,
            id => Self::Id(id),
        }
    }

    /// From a slot status's `bank_id`, the one field the proto makes optional.
    pub(crate) fn from_status(id: Option<u64>) -> Self {
        id.map_or(Self::Unknown, Self::from_wire)
    }

    /// Whether a message of `bank` belongs to the slot whose kept bank is `self`; an unknown bank
    /// on either side cannot be told apart, so it is admitted.
    pub(crate) fn admits(self, bank: Bank) -> bool {
        match (self, bank) {
            (Self::Id(kept), Self::Id(bank)) => kept == bank,
            _ => true,
        }
    }
}
