/// A field the decoder could not read from an account it still accepted: the pool is written
/// without it. Each one reaches its own log line and metric in yog-context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolAccountDegradation {
    /// A base-fee mode this build cannot map: `base_fee_kind` is not updated, the curve empty.
    BaseFeeModeUnknown,

    /// Too short for the optional tail (fee curve, price range, permanent lock): those stay empty.
    AccountTooShort { len: usize, needed: usize },
}
