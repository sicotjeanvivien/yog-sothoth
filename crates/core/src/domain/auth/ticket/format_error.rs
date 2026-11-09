use thiserror::Error;

use crate::domain::TICKET_SECRET_BYTES;

/// Why a presented string is not a ticket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum TicketFormatError {
    #[error("a ticket starts with `yogt_`")]
    Marker,
    #[error("the secret of a ticket is {} hex characters", TICKET_SECRET_BYTES * 2)]
    SecretLength,
    #[error("a ticket is lowercase hex after its marker")]
    NotLowerHex,
}
