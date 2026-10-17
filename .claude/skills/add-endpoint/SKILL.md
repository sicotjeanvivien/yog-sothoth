---
name: add-endpoint
description: Add a new read API endpoint to Yog-Sothoth (axum). Use when the user wants to expose existing data over HTTP — no new protocol, no new tables, no new domain types. Walks the contained core→persistence→api workflow from crates/README.md with real file paths, and respects cursor pagination, RFC 9457 errors, and the least-privilege RO api role.
---

# Add a new API endpoint

Authoritative recipe: `crates/README.md` → *Adding a new API endpoint*. This skill is the
operational checklist with real file paths.

Scope: this is for endpoints that **read existing data** — no new tables, no new domain
types, no new protocol. If you need a new protocol/product end-to-end, use `/add-protocol`
instead.

Templates to copy: the token read path —
`crates/api/src/application/services/token_service.rs`,
`crates/api/src/http/handlers/token.rs`, and the matching DTOs under
`crates/api/src/http/dto/`.

## Before you start — clarify scope

Read the rules first — `crates/api/README.md` → *What a route may return*: what a route
may return, how it is named, and the 2 s it must answer in.

Confirm with the user (don't assume):
- The **route** (e.g. `GET /api/tokens/{mint}`) and its path/query params.
- Whether the data is **cross-protocol** (read a VIEW: `swap_events`, `liquidity_events`, …)
  or **protocol-specific** (read the per-protocol table directly).
- Whether the query it needs **already exists** on a repository trait, or must be added.

## Step 1 — `core`: extend the repository trait (only if the query is new)

If the endpoint needs a query that doesn't exist:
- Add the method to the trait in `crates/core/src/domain/<aggregate>/repository.rs`.
- **Document the ordering and pagination contract** in the trait doc comment (the cursor
  type, the sort key, the tie-breaker).
- Pagination uses `Page<T>` (`crates/core/src/tools/pagination.rs`) + a domain-specific
  cursor type. Keep `core` infra-neutral: `Pubkey`, `rust_decimal::Decimal`, no
  `sqlx::types`.

If the query already exists, skip to Step 3.

## Step 2 — `persistence`: implement the method

- Add the SQL in the corresponding `Pg*Repository` impl under
  `crates/persistence/src/repositories/…`. Follow the existing
  `Row + TryFrom<XxxRow> for XxxDomain` convention.
- Map errors to `RepositoryError` at the boundary.
- **Regenerate the SQLx cache** (mandatory — CI's `sqlx-check` fails otherwise):
  ```bash
  cd crates/persistence && cargo sqlx prepare -- --all-targets --all-features
  ```
  Commit the updated `crates/persistence/.sqlx/`.
- Remember the privilege model: the api process connects as the **read-only `yog_api`
  role**. SELECT works everywhere; any INSERT/UPDATE from this path fails with
  `permission denied` **by design** — endpoints must be read-only.

## Step 3 — `api`: service, handler, DTOs, route

- **Service** — add/extend a module under
  `crates/api/src/application/services/<resource>_service.rs`
  (template: `token_service.rs`). The service holds the repo trait object
  and contains the read logic; the handler stays thin.
- **DTOs** — request DTO under `crates/api/src/http/dto/request/`, response DTO under
  `crates/api/src/http/dto/response/`, named and placed as *Where a response type goes*
  says (`crates/api/README.md`). Validate client-supplied data in the request DTO,
  before any DB call. The response DTO derives `utoipa::ToSchema`, the query struct
  `IntoParams`; a `Decimal`, an `i64`, a closed set of strings each need their wire form
  (`crates/api/README.md` → *The contract*).
- **Handler** — add to `crates/api/src/http/handlers/<aggregate>.rs`. Reuse `ApiError`
  (`crates/api/src/http/error.rs`); its `From<RepositoryError>` impl maps repo failures to
  RFC 9457 Problem Details uniformly — don't hand-roll error responses.
- **Wire a new service** into `AppState`: its field in `crates/api/src/http/state.rs`,
  its construction in `build_app_state` (`crates/api/src/bootstrap/app_state.rs`), where the
  concrete `Pg*` repositories are injected. Two files, two layers — the state belongs to
  the HTTP layer, its wiring to the bootstrap.
- **Declare and mount the route** — `#[utoipa::path(get, path = "/api/…", params(…),
  responses((status = 200, body = …)))]` on the handler, then
  `.routes(routes!(handlers::…))` in `api_routes` (`crates/api/src/http.rs`), in the
  slotted router if it is slow. Never `.route("/api/…")`: it serves the route without a
  contract, and a test fails on it.
- **Regenerate the contract**, and commit both:
  ```bash
  YOG_WRITE_OPENAPI=1 cargo test -p yog-api contract
  cd web && npm run gen:api
  ```

## Conventions (enforced)

- **Pagination** — collection endpoints use opaque base64 cursor pagination via `Page<T>`
  and a domain cursor type. Default `limit = 50`, hard cap `200`. Cursor wire format:
  `crates/api/src/http/cursor.rs`.
- **Errors** — RFC 9457 Problem Details (`application/problem+json`). 500s correlate via the
  `x-request-id` header.
- **Pubkeys** — base58 strings in/out (matching `Pubkey::Display`).
- **Timestamps** — RFC3339 / ISO8601.
- **Layering** — the binary wires repositories into the runtime; business logic lives in
  the service; the handler does request parsing + response shaping only. Handlers depend on
  repository **traits**, never `Pg*` concrete types.

## Step 4 — Verify

```bash
cargo fmt --all
cargo clippy -p yog-api -p yog-core -p yog-persistence --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
# Then run it against a live DB and hit the route:
cargo run -p yog-api
curl http://127.0.0.1:5000/api/<your-endpoint> | jq
```

## Definition of done

- [ ] Repository trait method added + ordering/pagination contract documented (if new)
- [ ] `Pg*` impl added; `.sqlx/` regenerated and committed
- [ ] Service + request/response DTOs created; input validated at the handler boundary
- [ ] Route declared with `#[utoipa::path]` and mounted in `api_routes` (`http.rs`)
- [ ] `crates/api/openapi.json` and `web/src/lib/api/schema/` regenerated and committed
- [ ] Cursor pagination + RFC 9457 errors respected; endpoint is read-only (RO `yog_api`)
- [ ] fmt / clippy (-D warnings) / tests green; route returns expected JSON via curl
