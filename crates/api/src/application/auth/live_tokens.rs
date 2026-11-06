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

/// The most tokens a client may issue in [`LIVE_TOKEN_TTL_SECS`]: past it, that client is refused
/// rather than memory spent, and the other clients keep theirs.
pub(crate) const MAX_LIVE_TOKENS_PER_CLIENT: usize = 10_000;

const MARKER: &str = "ylt_";
const SECRET_BYTES: usize = 32;

/// Each token not yet opened, by its SHA-256; every token issued, in the order it expires, with
/// its client; and how many each client issued that have not expired yet, opened or not.
#[derive(Default)]
struct Registry {
    grants: HashMap<[u8; 32], LiveGrant>,
    expiries: VecDeque<(i64, [u8; 32], Arc<str>)>,
    issued: HashMap<Arc<str>, usize>,
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
        let mut secret = [0u8; SECRET_BYTES];
        getrandom::fill(&mut secret).map_err(IssueError::Random)?;
        let token = format!("{MARKER}{}", hex(&secret));
        let key = digest(&token);
        let expires = now + i64::from(LIVE_TOKEN_TTL_SECS);
        let client: Arc<str> = client.into();

        let mut registry = self.registry();
        registry.forget_expired(now);
        let issued = registry.issued.entry(client.clone()).or_default();
        if *issued >= MAX_LIVE_TOKENS_PER_CLIENT {
            return Err(IssueError::Full);
        }
        *issued += 1;
        registry.grants.insert(
            key,
            LiveGrant {
                client: client.to_string(),
                scope: scope.clone(),
                expires,
            },
        );
        registry.expiries.push_back((expires, key, client));
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
    /// it finds expired, not for the whole registry. ⚠️ An opened token still counts to its
    /// client until it expires: the cap bounds what a client issues, so the queue too.
    fn forget_expired(&mut self, now: i64) {
        while let Some((expires, _, _)) = self.expiries.front() {
            if *expires > now {
                break;
            }
            let (_, key, client) = self.expiries.pop_front().expect("just read");
            self.grants.remove(&key);
            if let Some(count) = self.issued.get_mut(&client) {
                *count -= 1;
                if *count == 0 {
                    self.issued.remove(&client);
                }
            }
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    bytes
        .iter()
        .flat_map(|b| [DIGITS[usize::from(b >> 4)], DIGITS[usize::from(b & 0xf)]])
        .map(char::from)
        .collect()
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
