use std::env;

use crate::{
    endpoint::{Endpoint, KEY_PLACEHOLDER},
    error::ConfigError,
    secret::{SecretKey, SecretUrl},
};

/// Read a required environment variable, trimmed; absent, empty or blank is `MissingVariable`.
/// ⚠️ The trim strips the `\r` a CRLF `.env` leaks, for every helper built on this
/// one; `duration_var` trims on its own.
pub fn required(key: &str) -> Result<String, ConfigError> {
    match env::var(key).map(|v| v.trim().to_string()) {
        Ok(v) if !v.is_empty() => Ok(v),
        _ => Err(ConfigError::MissingVariable(key.to_string())),
    }
}

/// Read a required environment variable as a [`SecretUrl`].
/// ⚠️ Never a check reporting through [`ConfigError::InvalidValue`]: its `value`
/// would print the secret. The same binds every secret reader here.
pub fn required_secret_url(key: &str) -> Result<SecretUrl, ConfigError> {
    required(key).map(SecretUrl::new)
}

/// Read a required environment variable as a [`SecretKey`], a secret with no
/// carrier; fails only with `MissingVariable`, as [`required_secret_url`] says.
pub fn required_secret_key(key: &str) -> Result<SecretKey, ConfigError> {
    required(key).map(SecretKey::new)
}

/// Read an optional environment variable as a [`SecretUrl`], absent or blank read
/// as `None` by [`optional`]'s rule. It cannot fail, so no path from here reaches
/// a `value` field.
pub fn optional_secret_url(key: &str) -> Option<SecretUrl> {
    optional(key).map(SecretUrl::new)
}

/// Read an optional, non-secret environment variable, trimmed, blank read as absent.
/// ⚠️ Must agree with [`required`] on "set", or an empty `_KEY` passes
/// [`required_endpoint`]'s guard.
pub fn optional(key: &str) -> Option<String> {
    match env::var(key).map(|v| v.trim().to_string()) {
        Ok(v) if !v.is_empty() => Some(v),
        _ => None,
    }
}

/// Read the optional `<PREFIX>_HEADER_NAME` / `_VALUE` pair, refusing either half on
/// a consumer that would not send it, one half alone, and a name with whitespace, `:` or `{key}`.
/// ⚠️ The value is the credential: no error ever repeats it.
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

/// Read `<PREFIX>_URL` / `_KEY` for a consumer that sends **only the URL**.
/// ⚠️ The consumer chooses the door: a header pair is refused here, since nothing
/// would send it and the call would succeed anonymously.
pub fn required_endpoint(prefix: &str) -> Result<Endpoint, ConfigError> {
    read_endpoint(prefix, false)
}

/// Read an endpoint whose consumer **sends [`Endpoint::header`]**; the header is
/// allowed, not required, and `{key}` may sit in its value.
/// ⚠️ Calling it promises the consumer sends the header: nothing here can check it.
pub fn required_endpoint_allowing_header(prefix: &str) -> Result<Endpoint, ConfigError> {
    read_endpoint(prefix, true)
}

/// The body of both doors: refuses a `{key}` without `<PREFIX>_KEY`
/// (`MissingVariable`) and a key with no `{key}` to receive it; no `{key}` and no
/// key is a public endpoint. No value ever reaches [`ConfigError::InvalidValue`].
fn read_endpoint(prefix: &str, header_is_read: bool) -> Result<Endpoint, ConfigError> {
    let url_var = format!("{prefix}_URL");
    let name_var = format!("{prefix}_HEADER_NAME");
    let value_var = format!("{prefix}_HEADER_VALUE");
    let key_var = format!("{prefix}_KEY");

    let template = required(&url_var)?;
    let header = read_header(&name_var, &value_var, &url_var, header_is_read)?;
    let key = optional(&key_var);

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

/// Read an optional, non-secret variable as a `T` (the refusal prints it), `default`
/// when absent or blank; `T` carries the rule, a `NonZeroU32` refuses zero.
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

/// Read an optional `u64`, `default` when unset; it trims on its own.
/// ⚠️ A blank value is refused, not read as absent, unlike [`parse_optional`].
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

/// A configuration value read as one of a closed set of names; implementors
/// describe their names, `parse_required_enum` owns case and refusal.
pub trait EnvEnum: Sized {
    /// The accepted values, as the error message phrases them (`"rpc or grpc"`).
    const EXPECTED: &'static str;

    /// Map one accepted name to its variant.
    /// ⚠️ `value` arrives trimmed and lowercased: a non-lowercase literal is unreachable.
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
