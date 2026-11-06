//! The live tokens issued and not yet opened, held in memory: a token opens one stream once,
//! and leaves the registry as it does. ⚠️ One process: another instance of the API knows none.

use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex, MutexGuard},
};

use sha2::{Digest, Sha256};

use super::{issue_error::IssueError, live_grant::LiveGrant};
use crate::application::{LiveScope, Unauthorized};

/// How long a live token opens its stream, in seconds.
pub(crate) const LIVE_TOKEN_TTL_SECS: u32 = 30;

/// The most tokens waiting to be opened: past it, a token is refused rather than memory spent.
pub(crate) const MAX_LIVE_TOKENS: usize = 10_000;

const MARKER: &str = "ylt_";
const SECRET_BYTES: usize = 32;

/// Each token not yet opened, by its SHA-256, and every token issued in the order it expires.
#[derive(Default)]
struct Registry {
    grants: HashMap<[u8; 32], LiveGrant>,
    expiries: VecDeque<(i64, [u8; 32])>,
}

/// The tokens waiting to be opened, with what each grants. Clones share the registry.
#[derive(Clone, Default)]
pub(crate) struct LiveTokens {
    registry: Arc<Mutex<Registry>>,
}

impl LiveTokens {
    /// A token opening `scope` once for `client`, until `now` + [`LIVE_TOKEN_TTL_SECS`].
    pub(crate) fn issue(
        &self,
        client: &str,
        scope: &LiveScope,
        now: i64,
    ) -> Result<String, IssueError> {
        let mut registry = self.registry();
        registry.forget_expired(now);
        if registry.grants.len() >= MAX_LIVE_TOKENS {
            return Err(IssueError::Full);
        }
        let mut secret = [0u8; SECRET_BYTES];
        getrandom::fill(&mut secret).map_err(IssueError::Random)?;
        let token = format!(
            "{MARKER}{}",
            secret
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        );
        let key = digest(&token);
        let expires = now + i64::from(LIVE_TOKEN_TTL_SECS);
        registry.grants.insert(
            key,
            LiveGrant {
                client: client.to_string(),
                scope: scope.clone(),
                expires,
            },
        );
        registry.expiries.push_back((expires, key));
        Ok(token)
    }

    /// The client of `token`, which this opening of `asked` takes out of the registry.
    /// ⚠️ The scope is checked before it is taken out: tried elsewhere, a token still opens its own.
    pub(crate) fn open(
        &self,
        token: &str,
        asked: &LiveScope,
        now: i64,
    ) -> Result<String, Unauthorized> {
        if !is_token(token) {
            return Err(Unauthorized::TokenMalformed);
        }
        let key = digest(token);
        let mut registry = self.registry();
        let grant = registry
            .grants
            .get(&key)
            .ok_or(Unauthorized::TokenUnknown)?;
        if now >= grant.expires {
            registry.grants.remove(&key);
            return Err(Unauthorized::TokenExpired);
        }
        if !grant.scope.covers(asked) {
            return Err(Unauthorized::TokenScope);
        }
        let grant = registry
            .grants
            .remove(&key)
            .expect("read under the same lock");
        registry.forget_expired(now);
        Ok(grant.client)
    }

    fn registry(&self) -> MutexGuard<'_, Registry> {
        self.registry
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl Registry {
    /// The tokens expired by `now`, opened or not, taken off the front: each call pays for those
    /// it finds expired, not for the whole registry.
    fn forget_expired(&mut self, now: i64) {
        while let Some(&(expires, key)) = self.expiries.front() {
            if expires > now {
                break;
            }
            self.expiries.pop_front();
            self.grants.remove(&key);
        }
    }
}

fn digest(token: &str) -> [u8; 32] {
    Sha256::digest(token.as_bytes()).into()
}

/// `ylt_` and 64 lowercase hex characters: the form [`LiveTokens::issue`] writes.
fn is_token(token: &str) -> bool {
    token.strip_prefix(MARKER).is_some_and(|secret| {
        secret.len() == SECRET_BYTES * 2
            && secret
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

#[cfg(test)]
#[path = "live_tokens_tests.rs"]
mod tests;
