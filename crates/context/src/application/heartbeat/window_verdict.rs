/// The verdict on a window: a source called without a single success fails it.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum WindowVerdict {
    Healthy,
    /// Names each failing source, by role.
    Failing(String),
}
