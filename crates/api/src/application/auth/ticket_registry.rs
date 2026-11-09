//! The tickets issued and not yet used, held in memory: one opens one live connection, within
//! [`TICKET_LIFETIME`] of its issue. A restart forgets them, as it already cuts every stream.

use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};

use chrono::{DateTime, Utc};
use tokio::time::Instant;
use yog_core::domain::{ApiClient, TICKET_SECRET_BYTES, Ticket};

use crate::application::{Unauthorized, metrics::ApiMetrics};

pub(crate) const TICKET_LIFETIME: Duration = Duration::from_secs(30);

struct Issued {
    client: ApiClient,
    expires: Instant,
}

/// ⚠️ Every ticket lives as long: the order of issue is the order of expiry, so the oldest are
/// always at the front of `in_issue_order`.
#[derive(Default)]
struct Held {
    by_hash: HashMap<[u8; 32], Issued>,
    in_issue_order: VecDeque<([u8; 32], Instant)>,
}

/// Every ticket issued in the last [`TICKET_LIFETIME`], by hash, with its client.
#[derive(Clone, Default)]
pub(crate) struct TicketRegistry {
    held: Arc<Mutex<Held>>,
}

impl TicketRegistry {
    /// A new ticket for `client`, and the instant it stops opening anything.
    pub(crate) fn issue(
        &self,
        client: ApiClient,
    ) -> Result<(Ticket, DateTime<Utc>), getrandom::Error> {
        let mut secret = [0u8; TICKET_SECRET_BYTES];
        getrandom::fill(&mut secret)?;
        let ticket = Ticket::from_random(secret);
        let now = Instant::now();
        let expires = now + TICKET_LIFETIME;
        let name = client.name.clone();
        {
            let mut held = self.held();
            held.forget_expired(now);
            held.by_hash
                .insert(ticket.hash(), Issued { client, expires });
            held.in_issue_order.push_back((ticket.hash(), expires));
        }
        ApiMetrics::record_ticket_issued(&name);
        Ok((ticket, Utc::now() + TICKET_LIFETIME))
    }

    /// The client of `ticket`, which this uses up: a second call finds nothing.
    pub(crate) fn redeem(&self, ticket: &Ticket) -> Result<ApiClient, Unauthorized> {
        let issued = self
            .held()
            .by_hash
            .remove(&ticket.hash())
            .ok_or(Unauthorized::Unknown)?;
        if Instant::now() >= issued.expires {
            return Err(Unauthorized::Expired);
        }
        Ok(issued.client)
    }

    fn held(&self) -> MutexGuard<'_, Held> {
        self.held
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.held().in_issue_order.len()
    }
}

impl Held {
    /// Drop the tickets expired at `now`, from the oldest, up to the first still valid.
    fn forget_expired(&mut self, now: Instant) {
        while let Some(&(hash, expires)) = self.in_issue_order.front() {
            if expires > now {
                break;
            }
            self.in_issue_order.pop_front();
            self.by_hash.remove(&hash);
        }
    }
}

#[cfg(test)]
#[path = "ticket_registry_tests.rs"]
mod tests;
