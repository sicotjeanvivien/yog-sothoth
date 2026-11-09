use std::time::Duration;

use yog_core::domain::{ApiClient, Ticket};

use super::*;
use crate::testing::{counter, recorded};

fn dashboard() -> ApiClient {
    ApiClient {
        id: 7,
        name: "dashboard".to_string(),
    }
}

/// Mutation: `redeem` returning another client, or `issue` keeping another hash than the ticket's.
#[tokio::test(start_paused = true)]
async fn an_issued_ticket_names_its_client() {
    let registry = TicketRegistry::default();
    let (ticket, _) = registry.issue(dashboard()).expect("random bytes");

    assert_eq!(registry.redeem(&ticket), Ok(dashboard()));
}

/// Mutation: the ticket left in the registry once redeemed.
#[tokio::test(start_paused = true)]
async fn a_ticket_opens_once() {
    let registry = TicketRegistry::default();
    let (ticket, _) = registry.issue(dashboard()).expect("random bytes");
    registry.redeem(&ticket).expect("its first use");

    assert_eq!(registry.redeem(&ticket), Err(Unauthorized::Unknown));
}

/// Mutation: a ticket the registry never issued accepted.
#[tokio::test(start_paused = true)]
async fn a_ticket_never_issued_is_unknown() {
    let registry = TicketRegistry::default();
    registry.issue(dashboard()).expect("random bytes");

    let forged = Ticket::from_random([9; 32]);
    assert_eq!(registry.redeem(&forged), Err(Unauthorized::Unknown));
}

/// Mutation: the lifetime not checked on redeem, or checked with `>` (one more instant).
#[tokio::test(start_paused = true)]
async fn a_ticket_expires_at_its_lifetime() {
    let registry = TicketRegistry::default();
    let (early, _) = registry.issue(dashboard()).expect("random bytes");
    let (late, _) = registry.issue(dashboard()).expect("random bytes");

    tokio::time::advance(TICKET_LIFETIME - Duration::from_millis(1)).await;
    assert_eq!(registry.redeem(&early), Ok(dashboard()));
    tokio::time::advance(Duration::from_millis(1)).await;
    assert_eq!(registry.redeem(&late), Err(Unauthorized::Expired));
}

/// Mutation: `forget_expired` not called on issue, or stopping before the expired ones.
#[tokio::test(start_paused = true)]
async fn an_issue_forgets_the_expired_tickets() {
    let registry = TicketRegistry::default();
    registry.issue(dashboard()).expect("random bytes");
    registry.issue(dashboard()).expect("random bytes");

    tokio::time::advance(TICKET_LIFETIME).await;
    registry.issue(dashboard()).expect("random bytes");

    assert_eq!(registry.len(), 1);
}

/// Mutation: `forget_expired` going past the first ticket still valid.
#[tokio::test(start_paused = true)]
async fn an_issue_keeps_the_tickets_still_valid() {
    let registry = TicketRegistry::default();
    registry.issue(dashboard()).expect("random bytes");
    tokio::time::advance(Duration::from_secs(10)).await;
    let (valid, _) = registry.issue(dashboard()).expect("random bytes");

    tokio::time::advance(TICKET_LIFETIME - Duration::from_secs(5)).await;
    registry.issue(dashboard()).expect("random bytes");

    assert_eq!(registry.len(), 2);
    assert_eq!(registry.redeem(&valid), Ok(dashboard()));
}

/// Mutation: `expiresAt` another instant than the issue plus the lifetime.
#[tokio::test(start_paused = true)]
async fn a_ticket_says_when_it_expires() {
    let before = Utc::now();
    let (_, expires_at) = TicketRegistry::default()
        .issue(dashboard())
        .expect("random bytes");
    let after = Utc::now();

    assert!(expires_at >= before + TICKET_LIFETIME && expires_at <= after + TICKET_LIFETIME);
}

/// Mutation: two issues drawing the same bytes.
#[tokio::test(start_paused = true)]
async fn each_ticket_is_drawn_anew() {
    let registry = TicketRegistry::default();
    let (one, _) = registry.issue(dashboard()).expect("random bytes");
    let (two, _) = registry.issue(dashboard()).expect("random bytes");

    assert_ne!(one, two);
}

/// Mutation: an issue not counted, or counted under another client.
#[test]
fn an_issue_is_counted_under_its_client() {
    let ((), counters) = recorded(async {
        TicketRegistry::default()
            .issue(dashboard())
            .expect("random bytes");
    });

    assert_eq!(
        counter(
            &counters,
            "yog_api_tickets_issued_total",
            &[("client", "dashboard")]
        ),
        Some(1)
    );
}
