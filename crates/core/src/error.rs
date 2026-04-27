// core/src/error.rs
mod core_error;
mod repository_error;

pub use core_error::{CoreError, CoreResult};
pub use repository_error::{RepositoryError, RepositoryResult};