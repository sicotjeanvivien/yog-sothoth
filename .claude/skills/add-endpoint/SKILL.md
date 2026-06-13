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

Templates to copy: the swap/pool read path —
`crates/api/src/application/services/meteora_damm_v2_swap_service.rs`,
`crates/api/src/http/handlers/pools.rs`, and the matching DTOs under
`crates/api/src/http/dto/`.

## Before you start — clarify scope

Confirm with the user (don't assume):
- The **route** (e.g. `GET /api/pools/{address}/liquidity`) and its path/query params.
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
  cd crates/persistence && cargo sqlx prepare
  ```
  Commit the updated `crates/persistence/.sqlx/`.
- Remember the privilege model: the api process connects as the **read-only `yog_api`
  role**. SELECT works everywhere; any INSERT/UPDATE from this path fails with
  `permission denied` **by design** — endpoints must be read-only.

## Step 3 — `api`: service, handler, DTOs, route

- **Service** — add/extend a module under
  `crates/api/src/application/services/<platform>_<product>_<event_kind>_service.rs`
  (template: `meteora_damm_v2_swap_service.rs`). The service holds the repo trait object
  and contains the read logic; the handler stays thin.
- **DTOs** — request DTO under `crates/api/src/http/dto/request/`, response DTO under
  `crates/api/src/http/dto/response/`. Validate client-supplied data in the request DTO,
  before any DB call.
- **Handler** — add to `crates/api/src/http/handlers/<aggregate>.rs`. Reuse `ApiError`
  (`crates/api/src/http/error.rs`); its `From<RepositoryError>` impl maps repo failures to
  RFC 9457 Problem Details uniformly — don't hand-roll error responses.
- **Mount the route** in `build_router` in `crates/api/src/http.rs` (NB: the README says
  `http/mod.rs` — it's actually `http.rs`). Add the `.route("/api/…", get(...))` line next
  to the existing ones.

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
- [ ] Route mounted in `build_router` (`http.rs`)
- [ ] Cursor pagination + RFC 9457 errors respected; endpoint is read-only (RO `yog_api`)
- [ ] fmt / clippy (-D warnings) / tests green; route returns expected JSON via curl
