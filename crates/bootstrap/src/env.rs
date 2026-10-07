use std::env;

use crate::{
    endpoint::{Endpoint, KEY_PLACEHOLDER},
    error::ConfigError,
    secret::{SecretKey, SecretUrl},
};

/// Read a required environment variable, trimmed. Absent, empty or blank is
/// `MissingVariable`: `DATABASE_URL=` in a `.env` is an oversight, not a value.
///
/// ⚠️ The trim is here so that every helper built on this one inherits it: the
/// repository's `.env` has CRLF line endings, and sourcing it into a shell
/// leaks a `\r` that a parser then refuses for an invisible reason. It covers
/// the readers built on this one, not every environment read of the workspace
/// (see `duration_var`).
pub fn required(key: &str) -> Result<String, ConfigError> {
    match env::var(key).map(|v| v.trim().to_string()) {
        Ok(v) if !v.is_empty() => Ok(v),
        _ => Err(ConfigError::MissingVariable(key.to_string())),
    }
}

/// Read a required environment variable as a [`SecretUrl`] — the only way a
/// downstream crate gets one, since `SecretUrl::new` is `pub(crate)`.
///
/// ⚠️ It may only fail with `MissingVariable`, and must never gain a check that
/// reports through [`ConfigError::InvalidValue`]: that variant's `value` would
/// put the secret in the crash log. The same rule binds [`required_secret_key`].
pub fn required_secret_url(key: &str) -> Result<SecretUrl, ConfigError> {
    required(key).map(SecretUrl::new)
}

/// Read a required environment variable as a [`SecretKey`]: a value that is only
/// a secret, with no carrier worth showing. A URL whose host a startup failure
/// must name goes through [`required_secret_url`].
///
/// Fails only with `MissingVariable`, as [`required_secret_url`] says.
pub fn required_secret_key(key: &str) -> Result<SecretKey, ConfigError> {
    required(key).map(SecretKey::new)
}

/// Read an optional environment variable as a [`SecretUrl`], absent or blank read
/// as `None` by [`optional`]'s rule. It cannot fail, so no path from here reaches
/// a `value` field.
pub fn optional_secret_url(key: &str) -> Option<SecretUrl> {
    optional(key).map(SecretUrl::new)
}

/// Read an optional environment variable, trimmed, a blank value read as absent.
///
/// ⚠️ [`required`]'s rule minus the refusal: the two must agree on what "set"
/// means, or an empty `_KEY` would pass [`required_endpoint`]'s guard as if no
/// key were wanted. For a value that is not a secret; a secret goes through
/// [`required_secret_url`] or [`required_secret_key`].
pub fn optional(key: &str) -> Option<String> {
    match env::var(key).map(|v| v.trim().to_string()) {
        Ok(v) if !v.is_empty() => Some(v),
        _ => None,
    }
}

/// Read the optional `<PREFIX>_HEADER_NAME` / `<PREFIX>_HEADER_VALUE` pair.
///
/// Two variables, not one `name: value`: one would need a separator, a grammar
/// and its validation, and would be the workspace's only compound variable.
///
/// # What it refuses
///
/// - one half without the other → `UnsupportedCombination` naming both: a name
///   with no value sends an empty header, a value with no name has nowhere to go;
/// - a name carrying whitespace or a `:` → `UnsupportedCombination`: such a name
///   means nothing, and a `:` is `x-token: <secret>` pasted whole into the name,
///   which would print in the clear since a name is never masked;
/// - a `{key}` in the **name** → `UnsupportedCombination`: the placeholder is
///   substituted in the value only, so the name would reach the client verbatim;
/// - either half on an endpoint whose consumer sends only the URL → the refusal
///   described on [`required_endpoint`].
///
/// ⚠️ The name gets a shape check and the value none, on purpose: a name is a
/// token, so a space or a `:` in it means a broken line; a value is opaque
/// (`Bearer {key}`, a raw token), and checking it would be charset-guessing.
///
/// No value is ever repeated in an error: `<PREFIX>_HEADER_VALUE` carries the
/// credential, and the rule of [`required_secret_url`] binds here.
fn read_header(
    name_var: &str,
    value_var: &str,
    url_var: &str,
    header_is_read: bool,
) -> Result<Option<(String, String)>, ConfigError> {
    let name = optional(name_var);
    let value = optional(value_var);

    // ⚠️ This refusal comes first: telling an operator to complete a pair,
    // then to remove it, would send two refusals pointing opposite ways.
    if (name.is_some() || value.is_some()) && !header_is_read {
        return Err(ConfigError::UnsupportedCombination {
            detail: format!(
                "`{name_var}` / `{value_var}` is set, but the code that calls \
                 `{url_var}` sends only the URL — it would connect with no \
                 credential at all, and succeed anonymously against an endpoint \
                 that allows it. Unset them, or put the credential in \
                 `{url_var}` with a `{KEY_PLACEHOLDER}`"
            ),
        });
    }

    // ⚠️ The name's shape is checked whenever a name is present, not only on
    // a complete pair: otherwise `x-token: <secret>` pasted into the name is
    // told to complete the pair, and completing it earns the shape refusal.
    if let Some(name) = name.as_deref() {
        // Shape, not charset. `optional` trims the ends only.
        if name.contains(char::is_whitespace) || name.contains(':') {
            return Err(ConfigError::UnsupportedCombination {
                detail: format!(
                    "`{name_var}` is not a header name — it carries a space or a \
                     `:`. Write the name alone, and its value in `{value_var}`"
                ),
            });
        }
        if name.contains(KEY_PLACEHOLDER) {
            return Err(ConfigError::UnsupportedCombination {
                detail: format!(
                    "`{name_var}` carries `{KEY_PLACEHOLDER}`, which is only \
                     substituted in `{value_var}` — the header name would be \
                     sent verbatim. Write the placeholder in `{value_var}` \
                     instead"
                ),
            });
        }
    }

    match (name, value) {
        (None, None) => Ok(None),
        (Some(name), Some(value)) => Ok(Some((name, value))),
        (Some(_), None) => Err(ConfigError::UnsupportedCombination {
            detail: format!(
                "`{name_var}` is set without `{value_var}` — the header would be \
                 sent empty. Set both, or unset both"
            ),
        }),
        (None, Some(_)) => Err(ConfigError::UnsupportedCombination {
            detail: format!(
                "`{value_var}` is set without `{name_var}` — the value has no \
                 header to travel in. Set both, or unset both"
            ),
        }),
    }
}

/// Read an external endpoint whose consumer sends **only the URL** — the default
/// (`TOKEN_METADATA_*`, `POOL_ACCOUNT_*`, `INGEST_TRANSACTION_*`,
/// `NETWORK_STATUS_*`). The caller passes the prefix, and `<PREFIX>_URL` /
/// `<PREFIX>_KEY` are derived from it, so the convention holds at every site.
///
/// ⚠️ **The door is chosen by the consumer, never by the endpoint.**
/// `INGEST_STREAM_*` goes through [`required_endpoint_allowing_header`] because
/// its listeners send a header when one is configured. Without a pair, the two
/// doors give the same endpoint
/// (`without_a_header_the_two_doors_produce_the_same_endpoint`).
///
/// ⚠️ A `<PREFIX>_HEADER_NAME` / `_VALUE` is refused here, because nothing would
/// send it: a consumer passing [`Endpoint::url`] alone drops the header in
/// silence and authenticates as nobody, succeeding wherever anonymous callers
/// are tolerated.
pub fn required_endpoint(prefix: &str) -> Result<Endpoint, ConfigError> {
    read_endpoint(prefix, false)
}

/// Read an external endpoint whose consumer **calls [`Endpoint::header`]** and
/// sends what it returns.
///
/// ⚠️ It *allows* a header, it does not require one: without the pair it returns
/// what [`required_endpoint`] would. With it, `{key}` may be written in the
/// header value as well as in the URL, and is substituted wherever it is (see
/// [`crate::Endpoint`] for the shapes providers use).
///
/// ⚠️ Calling it is a promise that the consumer sends the header. Nothing here
/// can check it, which is why the two doors are two names rather than a boolean.
pub fn required_endpoint_allowing_header(prefix: &str) -> Result<Endpoint, ConfigError> {
    read_endpoint(prefix, true)
}

/// The shared body of [`required_endpoint`] and
/// [`required_endpoint_allowing_header`]. `header_is_read` is the caller stating
/// whether its consumer calls [`Endpoint::header`]: `yog-bootstrap` cannot know.
///
/// # What it refuses
///
/// - a `{key}` written somewhere and `<PREFIX>_KEY` absent or blank →
///   `MissingVariable` naming that variable: a literal `{key}` would otherwise
///   reach the address or the header, and the 401 would point nowhere;
/// - `<PREFIX>_KEY` set and no `{key}` in either carrier →
///   `UnsupportedCombination`: a credential configured for nowhere, and a process
///   authenticating as anonymous;
/// - the header pair's own refusals, via [`read_header`].
///
/// ⚠️ The first two are one rule read on two carriers: the match keeps four arms,
/// and a fifth would be a second rule.
///
/// No `{key}` and no key is a public endpoint, accepted as written. A `{key}` in
/// both carriers substitutes into both. No value ever reaches
/// [`ConfigError::InvalidValue`]: the rule of [`required_secret_url`] binds here.
fn read_endpoint(prefix: &str, header_is_read: bool) -> Result<Endpoint, ConfigError> {
    let url_var = format!("{prefix}_URL");
    let name_var = format!("{prefix}_HEADER_NAME");
    let value_var = format!("{prefix}_HEADER_VALUE");
    let key_var = format!("{prefix}_KEY");

    let template = required(&url_var)?;
    let header = read_header(&name_var, &value_var, &url_var, header_is_read)?;
    let key = optional(&key_var);

    // One placeholder, two possible carriers — see this function's docs.
    let has_placeholder = template.contains(KEY_PLACEHOLDER)
        || header
            .as_ref()
            .is_some_and(|(_, value)| value.contains(KEY_PLACEHOLDER));

    match (has_placeholder, key) {
        (true, Some(raw)) => Ok(Endpoint::new(template, header, Some(SecretKey::new(raw)))),
        (true, None) => Err(ConfigError::MissingVariable(key_var)),
        (false, None) => Ok(Endpoint::new(template, header, None)),
        // ⚠️ The advice depends on `header_is_read`: telling an operator to
        // write `{key}` in a header this endpoint refuses would send them into
        // the refusal above.
        (false, Some(_)) => Err(ConfigError::UnsupportedCombination {
            detail: if header_is_read {
                format!(
                    "`{key_var}` is set, but neither `{url_var}` nor `{value_var}` \
                     has a `{KEY_PLACEHOLDER}` to substitute it into — write \
                     `{KEY_PLACEHOLDER}` where the provider expects the credential, \
                     in the URL or in the header value, or unset `{key_var}` if the \
                     endpoint is public"
                )
            } else {
                format!(
                    "`{key_var}` is set, but `{url_var}` has no `{KEY_PLACEHOLDER}` \
                     to substitute it into — write `{KEY_PLACEHOLDER}` where the \
                     provider expects the credential, or unset `{key_var}` if the \
                     endpoint is public"
                )
            },
        }),
    }
}

/// Read a required environment variable as a `u32`: absent is `MissingVariable`,
/// unparseable `InvalidValue`. No default: a silent fallback would hide a typo.
pub fn parse_required_u32(key: &str) -> Result<u32, ConfigError> {
    let raw = required(key)?;
    raw.parse::<u32>().map_err(|_| ConfigError::InvalidValue {
        key: key.to_string(),
        value: raw,
        expected: "a non-negative integer (u32)",
    })
}

/// Read an optional environment variable and parse it as a `T`, falling back to
/// `default` when it is absent or blank, by [`optional`]'s rule.
///
/// The type carries the rule: a `NonZeroU32` refuses zero without the caller
/// restating it. `expected` is what the refusal tells the operator. For a value
/// that is not a secret: the refusal prints it.
///
/// ⚠️ Unlike [`duration_var`], a blank value falls back to the default.
pub fn parse_optional<T: std::str::FromStr>(
    key: &str,
    default: T,
    expected: &'static str,
) -> Result<T, ConfigError> {
    let Some(raw) = optional(key) else {
        return Ok(default);
    };
    raw.parse::<T>().map_err(|_| ConfigError::InvalidValue {
        key: key.to_string(),
        value: raw,
        expected,
    })
}

/// Read a required environment variable as a `bool`: `true` or `false`, in any
/// case. Anything else is refused, never coerced to `false`.
pub fn parse_required_bool(key: &str) -> Result<bool, ConfigError> {
    let raw = required(key)?;
    match raw.to_ascii_lowercase().as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(ConfigError::InvalidValue {
            key: key.to_string(),
            value: raw,
            expected: "true or false",
        }),
    }
}

/// Read an optional `u64` environment variable, falling back to `default` when
/// unset. A present but unparseable value is refused.
///
/// ⚠️ It trims on its own, since it cannot go through `required`, and a blank
/// value is refused rather than read as absent (unlike [`parse_optional`]).
///
/// ⚠️ No helper here covers every environment read of the workspace:
/// `tracing-subscriber` reads `RUST_LOG` inside its own crate. A new variable
/// goes through `required` if it is mandatory, and trims itself if not.
pub fn duration_var(key: &'static str, default: u64) -> Result<u64, ConfigError> {
    match std::env::var(key).map(|v| v.trim().to_string()) {
        Err(_) => Ok(default),
        Ok(raw) => raw.parse::<u64>().map_err(|_| ConfigError::InvalidValue {
            key: key.to_string(),
            value: raw,
            expected: "a integer (u64)",
        }),
    }
}

/// A configuration value read as one of a closed set of names — the alternative
/// to a `bool` when the axis has, or may have, more than two states.
/// Implementors describe their names; `parse_required_enum` owns case and
/// refusal.
pub trait EnvEnum: Sized {
    /// The accepted values, phrased as they appear in the error message
    /// (e.g. `"rpc or grpc"`).
    const EXPECTED: &'static str;

    /// Map one accepted name to its variant.
    ///
    /// `value` arrives **already trimmed** (by `required`) **and
    /// lowercased** — match on bare lowercase literals only, or the
    /// variant becomes unreachable.
    fn from_env_value(value: &str) -> Option<Self>;
}

/// Read a required environment variable as an [`EnvEnum`]: absent is
/// `MissingVariable`, an unknown name `InvalidValue` listing the accepted ones.
/// Case is folded here; `required` has already trimmed.
pub fn parse_required_enum<T: EnvEnum>(key: &str) -> Result<T, ConfigError> {
    let raw = required(key)?;
    T::from_env_value(&raw.to_ascii_lowercase()).ok_or_else(|| ConfigError::InvalidValue {
        key: key.to_string(),
        value: raw,
        expected: T::EXPECTED,
    })
}

#[cfg(test)]
#[path = "env_tests.rs"]
mod tests;
