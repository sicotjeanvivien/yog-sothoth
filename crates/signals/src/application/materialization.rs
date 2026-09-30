//! The materialisation alarm's rule: how a check of the backlogs ends, and
//! what a failure says. The loop that applies it is
//! [`MaterializationAlarm`](super::workers::MaterializationAlarm).

mod failure;
mod verdict;

pub(crate) use failure::{Failure, LateAggregate, hours_minutes};
pub(crate) use verdict::Verdict;
