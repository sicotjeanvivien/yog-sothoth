//! An endpoint's configuration, refused at start-up before anything is dialled: [`scheme`] checks
//! the address a path can speak, [`credential`] the header a request can carry.
//! Neither fault is caught by the client, which would report it as an unreachable provider.
//! ⚠️ Admission is "configures the endpoint", not "shared by both sources": `infra/refusal.rs`
//! is shared too and does not belong here.

mod credential;
pub(crate) mod scheme;

pub(crate) use credential::Credential;
