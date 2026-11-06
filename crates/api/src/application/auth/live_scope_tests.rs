use super::*;

fn scope(path: &str, topics: &[&str]) -> LiveScope {
    LiveScope {
        path: path.to_string(),
        topics: topics.iter().map(|t| t.to_string()).collect(),
    }
}

/// Mutation: the topics compared as an exact list, refusing an opening of fewer.
#[test]
fn a_scope_covers_its_path_with_its_topics_or_fewer() {
    let granted = scope("/api/live", &["signals", "pool-creations"]);

    assert!(granted.covers(&scope("/api/live", &["pool-creations", "signals"])));
    assert!(granted.covers(&scope("/api/live", &["signals"])));
}

#[test]
fn a_scope_does_not_cover_another_path() {
    let granted = scope("/api/live", &["signals"]);

    assert!(!granted.covers(&scope("/api/live/protocols/meteora_damm_v2", &["signals"])));
}

#[test]
fn a_scope_does_not_cover_a_topic_of_its_own() {
    let granted = scope("/api/live", &["signals"]);

    assert!(!granted.covers(&scope("/api/live", &["signals", "pool-creations"])));
}
