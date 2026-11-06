//! The live tokens issued and not yet opened, held in memory: a token opens one stream once,
//! and leaves the registry as it does. ⚠️ One process: another instance of the API knows none.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard},
};

use sha2::{Digest, Sha256};

use super::live_grant::LiveGrant;
use crate::application::{LiveScope, Unauthorized};

/// How long a live token opens its stream, in seconds.
pub(crate) const LIVE_TOKEN_TTL_SECS: u32 = 30;

const MARKER: &str = "ylt_";
const SECRET_BYTES: usize = 32;

type Grants = HashMap<[u8; 32], LiveGrant>;

/// Each token not yet opened, by its SHA-256, with what it grants. Clones share the registry.
#[derive(Clone, Default)]
pub(crate) struct LiveTokens {
    grants: Arc<Mutex<Grants>>,
}

impl LiveTokens {
    /// A token opening `scope` once for `client`, until `now` + [`LIVE_TOKEN_TTL_SECS`].
    pub(crate) fn issue(
        &self,
        client: &str,
        scope: &LiveScope,
        now: i64,
    ) -> Result<String, getrandom::Error> {
        let mut secret = [0u8; SECRET_BYTES];
        getrandom::fill(&mut secret)?;
        let token = format!(
            "{MARKER}{}",
            secret
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        );
        let mut grants = self.grants();
        forget_expired(&mut grants, now);
        grants.insert(
            digest(&token),
            LiveGrant {
                client: client.to_string(),
                scope: scope.clone(),
                expires: now + i64::from(LIVE_TOKEN_TTL_SECS),
            },
        );
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
        let mut grants = self.grants();
        let grant = grants.get(&key).ok_or(Unauthorized::TokenUnknown)?;
        if now >= grant.expires {
            grants.remove(&key);
            return Err(Unauthorized::TokenExpired);
        }
        if !grant.scope.covers(asked) {
            return Err(Unauthorized::TokenScope);
        }
        let grant = grants.remove(&key).expect("read under the same lock");
        forget_expired(&mut grants, now);
        Ok(grant.client)
    }

    fn grants(&self) -> MutexGuard<'_, Grants> {
        self.grants
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// ⚠️ Every call purges: the registry holds no more than the tokens of the last 30 s.
fn forget_expired(grants: &mut Grants, now: i64) {
    grants.retain(|_, grant| grant.expires > now);
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
