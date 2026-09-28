// A public type with no public path cannot be named by a caller, which
// can then neither match on it nor write it in a signature. Making a
// module private is how that happens here (crates/README.md, *Conventions*),
// and rustc only reports it under this lint, allowed by default.
#![warn(unnameable_types)]

pub mod amm;
pub mod application;
pub mod domain;
mod error;
mod tools;

pub use error::{AnchorDecodeError, CoreError, CoreResult, RepositoryError, RepositoryResult};

// Existing re-exports likely include this style — match it:
pub use tools::{Cursor, Page, PageDirection, PagePosition, PoolSort, PoolSortColumn};
