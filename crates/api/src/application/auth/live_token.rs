//! The live token: an HS256 JWT the API signs and checks itself, opening one scope once.

use std::sync::{Arc, Mutex};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use super::consumed::Consumed;
use crate::application::{LiveScope, Unauthorized};

/// How long a live token opens its stream, in seconds.
pub(crate) const LIVE_TOKEN_TTL_SECS: u32 = 30;

/// ⚠️ The only header accepted, compared as text: `alg` is never read from a token.
const HEADER: &str = r#"{"alg":"HS256","typ":"JWT"}"#;

#[derive(Serialize, Deserialize)]
struct Claims {
    sub: String,
    path: String,
    topics: Vec<String>,
    jti: String,
    iat: i64,
    exp: i64,
}

/// The API's signing key, and the ids already opened.
#[derive(Clone)]
pub(crate) struct LiveTokens {
    mac: Hmac<Sha256>,
    header: Arc<str>,
    consumed: Arc<Mutex<Consumed>>,
}

impl LiveTokens {
    pub(crate) fn new(secret: &[u8]) -> Self {
        Self {
            mac: Hmac::new_from_slice(secret).expect("HMAC takes a key of any length"),
            header: URL_SAFE_NO_PAD.encode(HEADER).into(),
            consumed: Arc::default(),
        }
    }

    /// A token opening `scope` once for `client`, until `now` + [`LIVE_TOKEN_TTL_SECS`].
    pub(crate) fn issue(
        &self,
        client: &str,
        scope: &LiveScope,
        now: i64,
    ) -> Result<String, getrandom::Error> {
        let mut id = [0u8; 16];
        getrandom::fill(&mut id)?;
        let claims = Claims {
            sub: client.to_string(),
            path: scope.path.clone(),
            topics: scope.topics.clone(),
            jti: id.iter().map(|b| format!("{b:02x}")).collect(),
            iat: now,
            exp: now + i64::from(LIVE_TOKEN_TTL_SECS),
        };
        let payload = serde_json::to_vec(&claims).expect("claims serialise");
        let signed = format!("{}.{}", self.header, URL_SAFE_NO_PAD.encode(payload));
        let signature = URL_SAFE_NO_PAD.encode(self.sign(&signed));
        Ok(format!("{signed}.{signature}"))
    }

    /// The client of `token`, consumed by this opening of `asked`.
    /// ⚠️ The scope is checked before the id is consumed: a token tried elsewhere is not spent.
    pub(crate) fn open(
        &self,
        token: &str,
        asked: &LiveScope,
        now: i64,
    ) -> Result<String, Unauthorized> {
        let mut parts = token.split('.');
        let (Some(header), Some(payload), Some(signature), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(Unauthorized::TokenMalformed);
        };
        if header != &*self.header {
            return Err(Unauthorized::TokenMalformed);
        }
        let signature = URL_SAFE_NO_PAD
            .decode(signature)
            .map_err(|_| Unauthorized::TokenMalformed)?;
        let mut mac = self.mac.clone();
        mac.update(format!("{header}.{payload}").as_bytes());
        mac.verify_slice(&signature)
            .map_err(|_| Unauthorized::TokenSignature)?;

        let claims: Claims = URL_SAFE_NO_PAD
            .decode(payload)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .ok_or(Unauthorized::TokenMalformed)?;
        if now >= claims.exp {
            return Err(Unauthorized::TokenExpired);
        }
        let granted = LiveScope {
            path: claims.path,
            topics: claims.topics,
        };
        if !granted.covers(asked) {
            return Err(Unauthorized::TokenScope);
        }
        let fresh = self
            .consumed
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .consume(&claims.jti, claims.exp, now);
        if !fresh {
            return Err(Unauthorized::TokenUsed);
        }
        Ok(claims.sub)
    }

    fn sign(&self, signed: &str) -> Vec<u8> {
        let mut mac = self.mac.clone();
        mac.update(signed.as_bytes());
        mac.finalize().into_bytes().to_vec()
    }
}

#[cfg(test)]
#[path = "live_token_tests.rs"]
mod tests;
