//! Response DTOs of the announcements.

mod entry_response;
mod kind_response;
mod severity_response;
mod status_response;

pub(crate) use entry_response::AnnouncementEntryResponse;

#[cfg(test)]
#[path = "tests/announcement_tests.rs"]
mod tests;
