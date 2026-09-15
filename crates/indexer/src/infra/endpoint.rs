//! Refuse a misconfigured provider endpoint at start-up, before anything is
//! dialled.
//!
//! # What the two modules here have in common
//!
//! An endpoint is an address and, sometimes, a credential. Neither is checked
//! by the client that will use it: `PubsubClient::new` accepts an `https://`
//! URL and simply fails to connect, tonic's `Endpoint::from_shared` accepts a
//! `wss://` one, and a header name that is not a token is only rejected when a
//! request is built. So each of these faults arrives **inside the retry loop**,
//! multiplied by the fleet, and reads like an unreachable provider.
//!
//! [`scheme`] answers "is this address one this path can speak", [`credential`]
//! answers "is this header one a request can carry", and both answer **once, at
//! start-up**, so the failure names the environment variable instead of burning
//! a retry budget. That is the subset: not "things both sources use", but the
//! configuration of the endpoint, verified before the first connection.
//!
//! Both also answer for **both** ingestion paths, and that is not a coincidence
//! either — the operator's configuration is not per-transport. Which schemes a
//! path accepts is what differs, and it is a [`scheme::Transport`], not a
//! module.
//!
//! # ⚠️ What does not belong here
//!
//! `infra/refusal.rs` is the other module both sources share, and it is
//! deliberately **not** in this folder. It is consumed at translation, on what
//! a provider sent, with nothing to do with reaching it. Filing it here would
//! group by "shared between the two sources" — a relationship, not a subject —
//! which is how a folder becomes a drawer. The rule for admission is the
//! paragraph above, not the number of callers.

mod credential;
pub(crate) mod scheme;

pub(crate) use credential::Credential;
