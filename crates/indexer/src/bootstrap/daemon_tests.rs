use super::*;

#[test]
fn handle_task_result_clean_stop_returns_ok() {
    let result: Result<Result<(), std::io::Error>, tokio::task::JoinError> = Ok(Ok(()));
    assert!(handle_task_result(result, "test task").is_ok());
}

#[test]
fn handle_task_result_task_error_returns_err() {
    let err = std::io::Error::other("boom");
    let result: Result<Result<(), std::io::Error>, tokio::task::JoinError> = Ok(Err(err));
    assert!(handle_task_result(result, "test task").is_err());
}
