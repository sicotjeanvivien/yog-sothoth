//! The single-use pass that opens one live connection: the registry only ever keeps its hash.

use std::fmt;

use sha2::{Digest, Sha256};

use crate::domain::TicketFormatError;
use crate::domain::auth::credential_text::{is_lower_hex, lower_hex};

pub const TICKET_SECRET_BYTES: usize = 32;

const MARKER: &str = "yogt_";

/// `yogt_<secret>`, in lowercase hex. ⚠️ `Debug` shows the marker alone, never the secret.
#[derive(Clone, PartialEq, Eq)]
pub struct Ticket {
    text: String,
}

impl Ticket {
    /// A new ticket from random bytes; the caller draws them, this crate does no I/O.
    pub fn from_random(secret: [u8; TICKET_SECRET_BYTES]) -> Self {
        Self {
            text: format!("{MARKER}{}", lower_hex(&secret)),
        }
    }

    pub fn parse(text: &str) -> Result<Self, TicketFormatError> {
        let secret = text.strip_prefix(MARKER).ok_or(TicketFormatError::Marker)?;
        if secret.len() != TICKET_SECRET_BYTES * 2 {
            return Err(TicketFormatError::SecretLength);
        }
        if !secret.bytes().all(is_lower_hex) {
            return Err(TicketFormatError::NotLowerHex);
        }
        Ok(Self {
            text: text.to_string(),
        })
    }

    /// The whole ticket, secret included: to hand to its client once, and never to log.
    pub fn expose(&self) -> &str {
        &self.text
    }

    /// The SHA-256 of the whole ticket, what the registry keeps.
    pub fn hash(&self) -> [u8; 32] {
        Sha256::digest(self.text.as_bytes()).into()
    }
}

impl fmt::Debug for Ticket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Ticket({MARKER}…)")
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
