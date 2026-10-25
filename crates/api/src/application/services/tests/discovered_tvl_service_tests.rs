//! Unit tests for `DiscoveredTvlService`: the reads a window not covered never makes.

use std::sync::Arc;

use yog_core::domain::Window;

use super::DiscoveredTvlService;
use crate::testing::{MockDiscoveredTvlRepo, at_october, frozen_hours_repo, make_frozen_hours};

/// Mutation: the `is_covered` guard dropped from the discoveries — a 30-day part over seven
/// days of facts.
#[tokio::test]
async fn the_discoveries_over_hours_the_facts_lack_are_not_read() {
    let repo = Arc::new(MockDiscoveredTvlRepo::default());
    let svc = DiscoveredTvlService::new(frozen_hours_repo(Some(make_frozen_hours())), repo.clone());

    svc.get_discovered_tvl(Window::Month, None).await.unwrap();
    svc.get_discovered_tvl(Window::Week, None).await.unwrap();

    let instants = repo.instants.lock().unwrap().clone();
    assert_eq!(instants, vec![("discoveries", at_october(20, 12), None)]);
}
