/// Why no live token was issued.
#[derive(Debug)]
pub(crate) enum IssueError {
    /// [`MAX_LIVE_TOKENS`](super::live_tokens::MAX_LIVE_TOKENS) are waiting to be opened.
    Full,
    /// The OS random source failed.
    Random(getrandom::Error),
}
