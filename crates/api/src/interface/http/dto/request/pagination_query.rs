use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use yog_core::{Cursor, domain::PoolCursor};

use crate::interface::HttpError;

/// Default page size when the client does not specify `limit`.
/// Suitable for a dashboard table — large enough to fill a screen,
/// small enough to keep transfers cheap.
const DEFAULT_LIMIT: i64 = 50;

/// Maximum value accepted from the client. The repository clamps to
/// the same upper bound, but rejecting at the parsing layer gives the
/// client a clearer 400 instead of silent truncation.
const MAX_LIMIT: i64 = 200;

/// Parsed pagination parameters from the query string.
///
/// Construct via `from_params`. The `cursor` is decoded eagerly so
/// invalid wire formats fail fast at the request boundary, before
/// any DB call.
#[derive(Debug)]
pub(crate) struct PaginationQuery {
    pub(crate) cursor: Option<PoolCursor>,
    pub(crate) limit: i64,
}

impl PaginationQuery {
    /// Parse pagination parameters from the request's query map.
    ///
    /// `cursor` is optional; if present, it must be a base64(url-safe,
    /// no-pad) encoded JSON of the cursor structure. `limit` is optional
    /// and defaults to `DEFAULT_LIMIT`; values outside `[1, MAX_LIMIT]`
    /// are rejected with `BadRequest`.
    pub(crate) fn from_params(params: &HashMap<String, String>) -> Result<Self, HttpError> {
        let cursor = match params.get("cursor") {
            Some(raw) if !raw.is_empty() => Some(decode_pool_cursor(raw)?),
            _ => None,
        };

        let limit = match params.get("limit") {
            None => DEFAULT_LIMIT,
            Some(raw) => raw
                .parse::<i64>()
                .map_err(|_| HttpError::BadRequest(format!("invalid `limit` value: {raw}")))?,
        };

        if limit < 1 || limit > MAX_LIMIT {
            return Err(HttpError::BadRequest(format!(
                "`limit` must be between 1 and {MAX_LIMIT}, got {limit}"
            )));
        }

        Ok(Self { cursor, limit })
    }
}

// ── Cursor wire format ───────────────────────────────────────────────────────
//
// The cursor is opaque from the client's perspective: it is a base64
// blob produced by a previous response and meant to be passed back
// unchanged. We use base64(url-safe, no-pad) over a JSON encoding of
// the cursor structure. JSON is verbose but keeps debugging easy
// (decode the base64 by hand, see what's inside) and lets us extend
// the cursor with new fields later without breaking the wire format.

#[derive(Debug, Serialize, Deserialize)]
struct PoolCursorWire {
    /// RFC3339 timestamp, e.g. "2026-04-30T14:32:11Z".
    first_seen_at: String,
    /// Base58 pubkey.
    pool_address: String,
}

/// Encode a `Cursor` into the wire format used in HTTP responses.
///
/// Currently only `Cursor::Pool` is supported; other variants will be
/// added as more domains become paginated.
pub(crate) fn encode_cursor(cursor: &Cursor) -> Result<String, HttpError> {
    match cursor {
        Cursor::Pool(c) => {
            let wire = PoolCursorWire {
                first_seen_at: c.first_seen_at.to_rfc3339(),
                pool_address: c.pool_address.to_string(),
            };
            let json = serde_json::to_vec(&wire).map_err(|e| {
                HttpError::InternalServerError(format!("failed to encode cursor: {e}"))
            })?;
            Ok(URL_SAFE_NO_PAD.encode(json))
        }
    }
}

/// Decode a wire-format cursor string into a `PoolCursor`.
fn decode_pool_cursor(raw: &str) -> Result<PoolCursor, HttpError> {
    use std::str::FromStr;

    let bytes = URL_SAFE_NO_PAD
        .decode(raw)
        .map_err(|_| HttpError::BadRequest("invalid cursor: not valid base64".to_string()))?;

    let wire: PoolCursorWire = serde_json::from_slice(&bytes)
        .map_err(|_| HttpError::BadRequest("invalid cursor: malformed payload".to_string()))?;

    let first_seen_at = chrono::DateTime::parse_from_rfc3339(&wire.first_seen_at)
        .map_err(|_| HttpError::BadRequest("invalid cursor: malformed timestamp".to_string()))?
        .with_timezone(&chrono::Utc);

    let pool_address = solana_pubkey::Pubkey::from_str(&wire.pool_address)
        .map_err(|_| HttpError::BadRequest("invalid cursor: malformed pool address".to_string()))?;

    Ok(PoolCursor {
        first_seen_at,
        pool_address,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_cursor() -> PoolCursor {
        use std::str::FromStr;
        PoolCursor {
            first_seen_at: chrono::DateTime::parse_from_rfc3339("2026-04-30T14:32:11Z")
                .unwrap()
                .with_timezone(&chrono::Utc),
            pool_address: solana_pubkey::Pubkey::from_str(
                "CGPxT5d1uf9a8cKVJuZaJAU76t2EfLGbTmRbfvLLZp5j",
            )
            .unwrap(),
        }
    }

    #[test]
    fn cursor_roundtrips_through_wire_format() {
        let original = sample_cursor();
        let encoded = encode_cursor(&Cursor::Pool(original.clone())).unwrap();
        let decoded = decode_pool_cursor(&encoded).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn limit_defaults_when_absent() {
        let q = PaginationQuery::from_params(&HashMap::new()).unwrap();
        assert_eq!(q.limit, DEFAULT_LIMIT);
        assert!(q.cursor.is_none());
    }

    #[test]
    fn limit_rejected_when_non_numeric() {
        let mut p = HashMap::new();
        p.insert("limit".to_string(), "abc".to_string());
        assert!(PaginationQuery::from_params(&p).is_err());
    }

    #[test]
    fn limit_rejected_when_out_of_range() {
        let mut p = HashMap::new();
        p.insert("limit".to_string(), "0".to_string());
        assert!(PaginationQuery::from_params(&p).is_err());

        p.insert("limit".to_string(), "201".to_string());
        assert!(PaginationQuery::from_params(&p).is_err());
    }

    #[test]
    fn invalid_cursor_rejected() {
        let mut p = HashMap::new();
        p.insert("cursor".to_string(), "not_base64!".to_string());
        assert!(PaginationQuery::from_params(&p).is_err());
    }
}
