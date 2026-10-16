// ⚠️ A private module can strand a public type no caller can name; rustc only
// reports it under this lint, allowed by default.
#![warn(unnameable_types)]

pub mod amm;
pub mod application;
pub mod domain;
mod error;
mod tools;

pub use error::{AnchorDecodeError, CoreError, CoreResult, RepositoryError, RepositoryResult};

pub use tools::{Cursor, Page, PageDirection, PagePosition, PoolSort, PoolSortColumn};
