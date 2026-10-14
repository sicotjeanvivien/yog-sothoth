//! Each wire enum writes the string its domain enum names itself with: red if a
//! wire variant is renamed, or loses its `rename`, without its domain string.

use serde::Serialize;
use yog_core::domain;

use super::announcement::{AnnouncementKind, AnnouncementSeverity};
use super::liquidity_event::LiquidityEventKind;
use super::network_status::Freshness;
use super::pool_current_state::LastEventKind;
use super::signal::Severity;
use super::swap_event::TradeDirection;

fn wire(value: impl Serialize) -> String {
    serde_json::to_value(value)
        .expect("a wire enum serialises")
        .as_str()
        .expect("a wire enum is a JSON string")
        .to_string()
}

#[test]
fn severity_matches_the_domain() {
    use domain::Severity::*;
    for s in [Info, Warning, Critical] {
        assert_eq!(wire(Severity::from(s)), s.as_str());
    }
}

#[test]
fn announcement_kind_and_severity_match_the_domain() {
    use domain::AnnouncementKind::*;
    for k in [Maintenance, Incident, Release, Beta] {
        assert_eq!(wire(AnnouncementKind::from(k)), k.as_str());
    }
    use domain::AnnouncementSeverity as S;
    for s in [S::Info, S::Warning, S::Critical] {
        assert_eq!(wire(AnnouncementSeverity::from(s)), s.as_str());
    }
}

#[test]
fn freshness_matches_the_domain() {
    use domain::FreshnessStatus::*;
    for f in [Live, Delayed, Stale] {
        assert_eq!(wire(Freshness::from(f)), f.as_str());
    }
}

#[test]
fn trade_direction_matches_the_domain() {
    use domain::TradeDirection::*;
    for d in [AtoB, BtoA] {
        assert_eq!(wire(TradeDirection::from(d)), d.as_str());
    }
}

#[test]
fn liquidity_event_kind_matches_the_domain() {
    use domain::MeteoraDammV2LiquidityEventKind::*;
    for k in [Add, Remove] {
        assert_eq!(wire(LiquidityEventKind::from(k)), k.as_str());
    }
}

#[test]
fn last_event_kind_matches_the_domain() {
    use domain::LastEventKind::*;
    for k in [Swap, LiquidityAdd, LiquidityRemove] {
        assert_eq!(wire(LastEventKind::from(k)), k.as_str());
    }
}
