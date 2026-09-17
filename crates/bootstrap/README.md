# yog-bootstrap

Library. Shared startup utilities for the native binaries (`yog-indexer`,
`yog-context`, `yog-signals`, `yog-api`) and `yog-migrate`: env parsing
primitives, the two secret types and the `Endpoint` that holds an address apart
from its credential, `ConfigError`, `init_rustls()`, `init_tracing()` — and the
other end of the same lifecycle, the shared stop.

The decision rule for adding anything: *does this run identically in every
binary's `main()`?* If it varies even slightly, it stays in the binary. Each
binary keeps its own `Config` struct; only the building blocks live here.

For the workspace-level picture (dependency graph, conventions, database
roles), see [`crates/README.md`](../README.md).

---

## Layout

```
bootstrap/src/
├── env.rs        ← required*, required_endpoint*, duration_var,
│                   parse_required_* — trim and blank-as-missing live in
│                   `required`, and the helpers built on it inherit them
├── secret.rs     ← SecretUrl, SecretKey — redaction, scrub, expose()
├── endpoint.rs   ← Endpoint: <FUNCTION>_URL + optional header pair + key
├── error.rs      ← ConfigError, returned by every binary's Config::load
├── runtime.rs    ← init_rustls(), init_tracing() and its filter
├── shutdown.rs   ← shutdown_signal(), TaskEnd, handle_task_result, Stop,
│                   SHUTDOWN_GRACE
└── lib.rs        ← re-exports, and the exposure_tests.rs guard
```

`env`, `secret`, `endpoint`, `runtime` and `shutdown` each have their
`*_tests.rs` beside them; `exposure_tests.rs` is the build-failing guard on
`.expose()` call sites (see *Secrets* below).

## The shared stop

`shutdown.rs` holds `shutdown_signal()` (SIGINT **and** SIGTERM), `TaskEnd`
(what a `JoinError` actually says), `handle_task_result`, and `Stop` with its
`SHUTDOWN_GRACE`.

⚠️ The stop moved here on 14 September 2026 and **not before**: `yog-indexer`
had it first, `yog-context` had grown its own copy of `handle_task_result`
whose doc-comment claimed it covered the same cases — and which had since
diverged. Two real users is what settled the shape. One consequence it had to
answer for: every line of a graceful stop now carries the log target
`yog_bootstrap`, and `EnvFilter` has no implicit global level — so a
`RUST_LOG` of per-crate directives printed none of them. `build_filter` keeps
that target audible unless the operator has said something that covers it.
Raised in review of PR #145, a second instance of the same thing: each daemon
guarded every `settle` call with an `if ended != Some(…)` — the rule that a
handle the `select!` already collected must not be polled again, written six
times across two binaries. `Stop::new` takes that name now and `settle` steps
over it, so a call site lists its stages in the order it wants them served and
says nothing else.

## Secrets — one invariant, two types

*The secret part is never printable; only the non-secret carrier is.*
`SecretKey` masks its value as `****` unconditionally, for a bare key or token.
`SecretUrl` redacts every component a URL can carry a credential in —
userinfo, path, query string, fragment — and keeps scheme, host and port, so a
daemon that dies on startup still says which provider it could not reach.
Postgres alone also keeps its role name and its database name: both are ours,
and both are the diagnostic.

**The path rule fails closed**, and that is the design: it is redacted for
every scheme *except* Postgres, whose path is the database name. So a provider
that puts its key in a path segment — Alchemy's `/v2/<key>`, QuickNode's
`/<token>/` — is covered by default rather than by having been recognised. An
earlier shape of this function knew only about `?`; that is precisely how it
came to print a bare API key in the clear.

So does the userinfo rule. When an `@` sits past the authority bound, the
password carries an unencoded delimiter *or* a path segment contains an `@`,
and nothing short of a URL parser tells the two apart — so the value comes back
as the scheme alone, `postgresql://***REDACTED***`. It costs a diagnostic on a
URL that hid nothing (`https://host:8080/pa@th`), and that is the trade: an
earlier attempt to disambiguate instead printed `postgresql://yog:pa#ss@…` in
full.

Neither is constructible outside the crate: a `Config` gets one from
`required_secret_url` / `required_secret_key`, and by no other route, so "a
secret is wrapped" is a compiler guarantee rather than a habit repeated at nine
sites. `expose()` is the one door out, and it belongs **on the line that
consumes the secret** — a `connect`, a request builder, a third-party client
constructor. The type travels there; it does not stop at the wiring.
`crates/bootstrap/src/exposure_tests.rs` fails the build on any exposure
outside that list.

Reaching for `SecretUrl` because it is the one that exists is how
`JUPITER_API_KEY` came to sit in a type that could not redact it: a bare key
has no `?`, so nothing was redacted.

### `SecretUrl::scrub` covers what a third party wrote

`Display` protects the value we hold; it does nothing for a `reqwest` error
that embedded the URL in its own message. `scrub` removes *its own* secret from
such a string — derived from the redaction rules rather than re-parsed, so it
cannot drift from them — and it is applied where that string is born, never at
the log site. It replaced `yog-indexer`'s `redact_api_key`, which matched the
literal `api-key=` and was blind to a credential in a path.

## Endpoints — named after what they serve, credential outside the URL

`Endpoint` is a `<FUNCTION>_URL` plus an optional `<FUNCTION>_HEADER_NAME` /
`<FUNCTION>_HEADER_VALUE` pair, and the `<FUNCTION>_KEY` they share — read by
`required_endpoint("<FUNCTION>")`, which derives every name from the one
prefix. Three of the four endpoints in place today — `TOKEN_METADATA_*`,
`POOL_ACCOUNT_*`, `INGEST_TRANSACTION_*` — go through that door and therefore
**refuse** a header pair. The fourth, `INGEST_STREAM_*`, goes through
`required_endpoint_allowing_header`, because both of its listeners send what it
carries: `yog-indexer`'s `infra::endpoint::credential` is the single place that
turns the pair into something a client sends, as gRPC request metadata or as a
WebSocket handshake header.

⚠️ **What decides whether there is a header is those two variables and nothing
else** — not `INGEST_SOURCE`. A door chosen on the transport was tried for a
day and removed on review: it rested on "the WebSocket client cannot send one",
which is false (`PubsubClient::new` takes an `IntoClientRequest`), and it put a
transport in charge of a credential question — the same inversion this family
of tickets removed from variable *names*. The door grants the ability to carry
a credential; it does not impose one, and three of the four measured provider
shapes send none.

The operator writes `{key}` where the provider expects its credential, and the
halves are joined only in `Endpoint::url()` and `Endpoint::header()`, which
return a `SecretUrl` and a `SecretKey` because from there on the value does
carry the key. Three consequences worth the trouble.

- **A name that describes a *transport* excludes nothing** — `SOLANA_RPC_HTTP`
  had silently come to serve three roles across two crates, and one variable
  cannot hold two addresses the day a provider changes; a name that describes a
  *function* refuses on its own.
- **The operator writes the shape, so the code knows no provider**: Helius'
  `?api-key=`, Alchemy's `/v2/<key>`, QuickNode's `/<token>/` and the `x-token:`
  header most of them want for a gRPC stream are one line here — and the
  header's *name* is a provider convention too, which is why it is written and
  not compiled in — in its own variable, since every variable here holds
  exactly one thing and a compound one would need a grammar to validate. That
  is the same bet `redact_api_key` lost, and it lands the other way for a
  nameable reason — **knowing a shape to *build* is safe, knowing a shape to
  *redact* is not**: a wrong template gives a frank 401 on the first call, a
  wrong redactor writes a secret in the clear and nobody sees it.
- **A credential only counts where somebody sends it**: `required_endpoint`
  **refuses** a header, because every consumer today passes `url()` alone and a
  dropped header authenticates as nobody; a consumer that really sends one says
  so by calling `required_endpoint_allowing_header`.

Every mismatch is refused at startup naming the variable — the list lives in
`.env.example`, uncounted on purpose, a hand-kept tally having drifted three
times — and `Display` fails closed on both carriers: a template with `{key}`
prints whole (that is the return — an address that hides nothing is legible in
a log), one without falls back on `redact`, since nothing can tell a public
endpoint from a credential somebody pasted in.
