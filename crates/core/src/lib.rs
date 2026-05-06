pub mod amm;
pub mod domain;
pub mod error;
pub mod protocols;
pub mod tools;

pub use error::{CoreError, CoreResult, RepositoryError, RepositoryResult};

// Existing re-exports likely include this style — match it:
pub use tools::{Cursor, Page};