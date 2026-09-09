use std::env;

use crate::{
    endpoint::{Endpoint, KEY_PLACEHOLDER},
    error::ConfigError,
    secret::{SecretKey, SecretUrl},
};

/// Read a required environment variable. Returns `MissingVariable` if
/// the key is absent, empty, or blank.
///
/// Empty strings are treated as missing on purpose — a `.env` line like
/// `DATABASE_URL=` is almost certainly an oversight, and silently
/// returning an empty value would propagate the bug deeper into the
/// system before failing.
///
/// Surrounding whitespace is trimmed **here**, so that every helper
/// built on this one inherits it rather than restating it: this
/// repository's `.env` has CRLF line endings, and the documented native
/// workflow sources it into the shell (`set -a; . ./.env`). A leaked
/// `\r` then reaches whichever parser reads the value, and produces a
/// refusal whose cause is an invisible byte — ``got `10`, expected a
/// non-negative integer`` — or, on a connection string, an opaque
/// network error. Trimming here is what lets every caller inherit the rule
/// instead of restating it; this doc deliberately does **not** claim to
/// cover every environment read in the workspace — that claim was made
/// three times and was wrong three times, see `duration_var`.
pub fn required(key: &str) -> Result<String, ConfigError> {
    match env::var(key).map(|v| v.trim().to_string()) {
        Ok(v) if !v.is_empty() => Ok(v),
        _ => Err(ConfigError::MissingVariable(key.to_string())),
    }
}

/// Read a required environment variable and wrap it as a [`SecretUrl`].
///
/// The only way a downstream crate obtains one: `SecretUrl::new` is
/// `pub(crate)`, so "a connection string is wrapped" is a fact the compiler
/// enforces rather than a habit each `Config` has to keep.
///
/// # This helper may only fail with `MissingVariable`
///
/// It deliberately does no validation, and it must never gain any that reports
/// through [`ConfigError::InvalidValue`] — that variant carries a `value`
/// field, and a secret placed there is a secret in the crash log, which is the
/// exact defect this module exists to close. `MissingVariable` carries the key
/// alone, which is why it is the only variant reachable from here. The same
/// rule binds [`required_secret_key`].
pub fn required_secret_url(key: &str) -> Result<SecretUrl, ConfigError> {
    required(key).map(SecretUrl::new)
}

/// Read a required environment variable and wrap it as a [`SecretKey`].
///
/// For a value that is *only* a secret — an API key, a token — with no carrier
/// worth showing. Use [`required_secret_url`] when the value is a URL whose
/// host and path a startup failure needs to name.
///
/// Fails only with `MissingVariable`, for the reason spelled out on
/// [`required_secret_url`].
pub fn required_secret_key(key: &str) -> Result<SecretKey, ConfigError> {
    required(key).map(SecretKey::new)
}

/// Read an optional environment variable, trimmed, with a blank value read as
/// absent.
///
/// The same rule [`required`] applies, minus the refusal: `FOO=` in a `.env` is
/// an oversight there and an oversight here, and the two must not disagree on
/// what "set" means — that disagreement is what would let an empty `_KEY` slip
/// past [`required_endpoint`]'s guard as if the operator had chosen not to
/// have one.
fn optional(key: &str) -> Option<String> {
    match env::var(key).map(|v| v.trim().to_string()) {
        Ok(v) if !v.is_empty() => Some(v),
        _ => None,
    }
}

/// Read the optional `<PREFIX>_HEADER_NAME` / `<PREFIX>_HEADER_VALUE` pair.
///
/// # Why two variables and not one `<PREFIX>_HEADER=name: value`
///
/// That is what this first shipped as, and it was withdrawn. A single variable
/// holding a name **and** a value needs a separator; a separator needs a
/// grammar; a grammar needs validating — and that came to 70 lines and five
/// refusals (missing `:`, empty name, whitespace in name, empty value, a `{key}`
/// written in the name), three of which review had to find rather than the
/// design prevent. One of them accepted a literal `{key}` as a header name in
/// silence.
///
/// It also made that variable the only one of the workspace's 45 to carry a
/// compound value — which is the very defect this family of tickets removes,
/// reproduced one scale down: `SOLANA_RPC_HTTP` held three roles and was split
/// into four pairs, and the fix then wrote a variable holding two.
///
/// Read as two, nothing is parsed. [`optional`] already gives the trim and
/// "blank is absent", so both are inherited rather than restated, and what is
/// left is three refusals of the kind this module already has — none of them a
/// grammar.
///
/// # What it refuses
///
/// - one half without the other → `UnsupportedCombination` naming both. A name
///   with no value sends an empty header; a value with no name has nowhere to
///   go. Neither is a shape anyone means;
/// - a name carrying whitespace or a `:` → `UnsupportedCombination`. That is
///   shape and not charset: such a name means nothing, and a `:` is the
///   signature of `x-token: <secret>` pasted whole into the name variable,
///   which would also print in the clear since a name is never masked;
/// - a `{key}` in the **name** → `UnsupportedCombination`. The placeholder is
///   substituted in the value and nowhere else, so a name carrying one reaches
///   the client verbatim as `{key}` — the "401 nothing connects back to the
///   configuration" this validation exists to prevent;
/// - either half set on an endpoint whose consumer sends only the URL → the
///   refusal described on [`required_endpoint`].
///
/// ⚠️ The **name** gets a shape check and the **value** gets none, and the
/// asymmetry is intended. A name is a token whose only job is to be one, so a
/// space or a `:` in it means the line is broken. A value is opaque by nature —
/// `Bearer {key}`, a raw token, anything a provider asks for — so there is no
/// shape to check that would not be charset-guessing. What a client will accept
/// is the client's affair, and it says so loudly on the first call.
///
/// No value is ever repeated in an error: `<PREFIX>_HEADER_VALUE` is what
/// carries the credential, and the rule binding [`required_secret_url`] binds
/// here.
fn read_header(
    name_var: &str,
    value_var: &str,
    url_var: &str,
    header_is_read: bool,
) -> Result<Option<(String, String)>, ConfigError> {
    let name = optional(name_var);
    let value = optional(value_var);

    // The "nobody would send it" refusal comes first, and the order is the
    // message: an operator told to complete a pair, and then told to remove it
    // entirely, has been sent two refusals pointing opposite ways.
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

    // ⚠️ The name's shape is checked whenever a name is present **at all**, and
    // not only when the pair is complete. Found in review, 8 September 2026:
    // with these checks living in the `(Some, Some)` arms, an operator who
    // pasted `x-token: <secret>` into `_HEADER_NAME` and had not yet written
    // `_HEADER_VALUE` was told to complete the pair — and completing it as
    // advised earned the shape refusal on the next start. That is the very
    // "a refusal that names a fix the next refusal undoes" defect this module
    // added a test for, reproduced one level down inside this function.
    if let Some(name) = name.as_deref() {
        // Shape, not charset — the boundary this module keeps. A space or a `:`
        // in a header name is not a subtlety of the HTTP token grammar, it is a
        // line that means nothing, and the second one is the shape of a specific
        // mistake: pasting `x-token: <secret>` into `_HEADER_NAME` after this
        // feature moved from one variable to two. Left accepted, that paste also
        // prints in the clear, since a name is never masked.
        //
        // The single-variable parser refused whitespace here and the split
        // dropped it; restored 8 September 2026 after review caught the
        // regression. `optional` trims the ends and nothing more.
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

/// Read an external endpoint whose consumer sends **only the URL**.
///
/// The default, and what three of the workspace's four endpoints use:
/// `TOKEN_METADATA_*`, `POOL_ACCOUNT_*` and `INGEST_TRANSACTION_*`, whose
/// consumers hand [`Endpoint::url`] to `reqwest` or `RpcClient` and nothing
/// else. The caller passes the **prefix**, and `<PREFIX>_URL` /
/// `<PREFIX>_KEY` are derived from it — one name, one place. Spelling them at
/// every call site is how a convention comes to hold at some sites and not
/// others, which is the defect this whole family of tickets is about.
///
/// ⚠️ **The choice of door is about the consumer, never about the endpoint.**
/// `INGEST_STREAM_*` goes through [`required_endpoint_with_header`] because
/// both of its listeners send a header when one is configured — not because a
/// stream needs one. With no pair configured the two doors are
/// indistinguishable, which is what makes the wider one safe for a variable
/// that *may* carry a header; pinned by
/// `without_a_header_the_two_doors_produce_the_same_endpoint`.
///
/// # Why a `<PREFIX>_HEADER_NAME` / `_VALUE` is refused here
///
/// Because nothing would send it. A header is only a credential if some code
/// downstream calls [`Endpoint::header`]; when the consumer passes
/// [`Endpoint::url`] alone — as `RpcClient`, `PubsubClient` and the two
/// `reqwest` providers do — a configured header is dropped in silence and the
/// process authenticates as **nobody**, succeeding against any endpoint that
/// tolerates anonymous callers.
///
/// That is the same silent-anonymous failure the "a key with nowhere to go"
/// refusal exists to prevent, and it would have been re-opened by treating a
/// header as a valid carrier everywhere. Found in review, 8 September 2026,
/// after a verification run that read `# Connected.` as a success when it was
/// the defect: the stream had connected with no credential at all.
///
/// A consumer that *does* read the header calls
/// [`required_endpoint_with_header`] instead. The distinction is the caller
/// stating a fact about itself, which is the only place that fact exists.
pub fn required_endpoint(prefix: &str) -> Result<Endpoint, ConfigError> {
    read_endpoint(prefix, false)
}

/// Read an external endpoint whose consumer **calls [`Endpoint::header`]**.
///
/// Identical to [`required_endpoint`] except that the optional
/// `<PREFIX>_HEADER_NAME` / `<PREFIX>_HEADER_VALUE` pair is accepted, with
/// `{key}` written in the **value** wherever the provider expects the
/// credential. The placeholder is then looked for in the URL **and** in the
/// header value, and substituted wherever the operator put it — see the
/// module docs of [`crate::Endpoint`] for the four shapes measured across
/// providers, and why the header's *name* is a provider convention too.
///
/// ⚠️ Calling this is a **promise**, not a preference: the code receiving the
/// `Endpoint` must actually send the header. Nothing here can check that, which
/// is exactly why the two doors are separate names rather than a boolean an
/// author sets without reading it.
pub fn required_endpoint_with_header(prefix: &str) -> Result<Endpoint, ConfigError> {
    read_endpoint(prefix, true)
}

/// The shared body of [`required_endpoint`] and
/// [`required_endpoint_with_header`].
///
/// `header_is_read` is what the two doors differ by, and it is the caller
/// stating a fact about **its own consumer**: whether the code downstream will
/// actually call [`Endpoint::header`]. `yog-bootstrap` cannot know that, and
/// guessing it is what produced the defect this parameter exists to close.
///
/// # What it refuses, and why each refusal is loud
///
/// - a `{key}` is written **somewhere** and `<PREFIX>_KEY` is absent or blank →
///   `MissingVariable`, **naming that variable**. Accepting it would start the
///   process with a literal `{key}` in its address or its header, and the
///   operator would then be reading a 401 that nothing connects back to the
///   configuration;
/// - `<PREFIX>_KEY` is set and **no** `{key}` exists in either carrier →
///   `UnsupportedCombination`. It is the same failure seen from the other side:
///   a credential that is configured and goes nowhere. Silence here means the
///   process authenticates as anonymous and the operator has no reason to
///   suspect it;
/// - one half of `<PREFIX>_HEADER_NAME` / `<PREFIX>_HEADER_VALUE` without the
///   other, or a `{key}` written in the name → `UnsupportedCombination`, via
///   [`read_header`].
///
/// ⚠️ The first two are **one rule read on two carriers**, not two rules. The
/// match below still has four arms, and that is deliberate: a fifth would mean
/// a second rule sitting beside the first, and a rule written twice is a rule
/// that holds at one site out of two.
///
/// No `{key}` anywhere and no key is a **public endpoint**, accepted exactly as
/// written — `api.mainnet-beta.solana.com` wants no credential.
///
/// A `{key}` in **both** carriers substitutes into both. It has no known use,
/// but it is well defined, and refusing it would add a rule where there is no
/// defect to correct.
///
/// Fails only with those variants and with the `MissingVariable` of the URL
/// itself: no value ever reaches [`ConfigError::InvalidValue`], whose `value`
/// field would put it in the crash log — the rule that binds
/// [`required_secret_url`] binds here too.
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
        // ⚠️ The advice is conditioned on `header_is_read`, and that is not
        // cosmetic: telling an operator to write `{key}` "in the header" on an
        // endpoint that refuses headers sends them straight into the refusal
        // above, which then tells them the opposite. A refusal that names a fix
        // the next refusal undoes is worse than one that names none.
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

/// Read a required environment variable and parse it as a `u32`.
///
/// Fails with `MissingVariable` if absent, `InvalidValue` if present
/// but unparseable. Silent fallback to a default would mask typos in
/// the `.env`.
pub fn parse_required_u32(key: &str) -> Result<u32, ConfigError> {
    let raw = required(key)?;
    raw.parse::<u32>().map_err(|_| ConfigError::InvalidValue {
        key: key.to_string(),
        value: raw,
        expected: "a non-negative integer (u32)",
    })
}

/// Read a required environment variable and parse it as a `bool`.
///
/// Accepts the literals `true` and `false` (case-insensitive). Anything
/// else is rejected — a loud failure is preferable to a silent coercion
/// to `false`.
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

/// Read an optional `u64` environment variable, falling back to
/// `default` when unset. A present-but-unparseable value is an error.
///
/// Trims for the same reason `required` does, which it cannot reuse: a value
/// carrying a default is allowed to be absent, and `required` refuses that.
/// Known siblings in the same position, trimming for the same reason:
/// `optional` just above, `decimal_var` in `yog-signals`, and `LOG_FORMAT` /
/// `RUST_LOG` in `init_tracing`.
///
/// **That list is not a guarantee, and no claim here should be read as one.**
/// It was asserted as exhaustive three times in one day and was wrong three
/// times — the last miss being `RUST_LOG`, which `tracing-subscriber` reads
/// *inside its own crate*, so no grep of this workspace's sources could have
/// found it. A reader adding an environment-backed value: route it through
/// `required` if it is mandatory, and trim it yourself if it is not.
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

/// A configuration value read from the environment as one of a closed
/// set of names — the alternative to a `bool` whenever the axis has, or
/// may one day have, more than two states.
///
/// Implementors describe *what* their names are. They do not deal with
/// case, nor with what an unknown value costs: `parse_required_enum`
/// owns both, so the rule is written once instead of being restated —
/// and forgotten — in every enum.
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

/// Read a required environment variable and parse it as an [`EnvEnum`].
///
/// Fails with `MissingVariable` if absent or empty, `InvalidValue` if
/// present but not one of the accepted names — which are listed back to
/// the operator, since a config that dies at startup should say what it
/// wanted instead of what it got.
///
/// Case is folded here; whitespace is already gone, trimmed by
/// `required` for every variable rather than by this helper for two.
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
