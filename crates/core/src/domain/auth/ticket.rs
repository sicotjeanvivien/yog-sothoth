mod format_error;
mod model;

pub use format_error::TicketFormatError;
pub use model::{TICKET_SECRET_BYTES, Ticket};
