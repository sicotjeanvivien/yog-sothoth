/// Why no live token was issued.
#[derive(Debug)]
pub(crate) enum IssueError {
    /// The client issued [`MAX_LIVE_TOKENS_PER_CLIENT`](super::live_tokens::MAX_LIVE_TOKENS_PER_CLIENT) in the last 30 s.
    Full,
    /// The OS random source failed.
    Random(getrandom::Error),
}
