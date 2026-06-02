# yog-sothoth-web

Next.js 16 frontend for the Yog-Sothoth liquidity intelligence engine.

This package lives next to the Rust crates of the project but is fully
independent at the Node.js level. It talks to `yog-api` over HTTP through
a thin BFF layer — it never connects to TimescaleDB directly.

## Stack

- **Next.js 16** — App Router, Server Components, standalone output, Turbopack by default
- **React 19.2** — bundled with Next 16
- **TypeScript** — strict mode enabled, including `noUncheckedIndexedAccess`
- **Tailwind CSS** — palette extracted from the Yog-Sothoth mockups
- **next-intl 4** — i18n with always-visible locale prefix (`/en/...`, `/fr/...`)
- **zod** — runtime validation of every payload returned by `yog-api`
- **Vitest** — unit tests in Node environment

## Architecture

```
┌─────────────┐    HTTP    ┌─────────────────────────┐    HTTP    ┌─────────┐
│   Browser   │───────────▶│  Next.js (this package) │───────────▶│ yog-api │
│             │            │                         │            │  (Rust) │
│             │            │  Server Components +    │            │         │
│             │            │  Route Handlers (BFF)   │            │         │
└─────────────┘            └─────────────────────────┘            └─────────┘
                                       │
                                       │ Direct DB read is NOT used in
                                       │ the current shape — yog_web is
                                       │ kept as a future fallback role.
                                       ▼
                              TimescaleDB (yog_web RO)
```

The frontend has two consumers of `yog-api`:

- **Server Components** — execute on the Node.js server, read `yog-api` via the
  typed client (`lib/api/`). They use `API_INTERNAL_URL` to talk over the Docker
  network (`http://yog-api:5000`).
- **The browser** — never calls `yog-api` directly. It calls **route handlers**
  under `app/api/` that act as a BFF (Backend For Frontend), proxying the request
  and translating HTTP errors into a stable, frontend-friendly shape.
- **`schema/api-error-body.ts`** — zod schema for the RFC 9457 Problem
  Details envelope returned by `yog-api` on errors. Used internally by
  `client.ts` to extract the `detail` field as the remote message
  attached to `ApiClientError.http(...)`. The BFF route handlers do
  not see this format — they see `ApiClientError` and produce their
  own browser-facing envelope.

This split protects the browser from internal details: 5xx responses from
`yog-api` are collapsed into a generic 502 by the BFF (no leakage of stack
traces or DB errors), while 4xx pass through unchanged because they describe a
client-side mistake the caller needs to know about.

## Talking to `yog-api`

The integration layer lives under `src/lib/api/`:

- **`client.ts`** — base fetch wrapper with timeout, AbortController, and zod
  schema validation. Returns a discriminated `Result`-like type.
- **`errors.ts`** — `ApiClientError` discriminated union with four variants:
  `timeout`, `unavailable`, `bad_request`, `unexpected`. Server Components and
  BFF handlers pattern-match on the variant to render the right state.
- **`pools.ts`, `tokens.ts`, …** — one module per resource, exposing a
  `fetchXxx()` (throwing) and a `safeFetchXxx()` (returning `Result`).

Server Components consume the `safeFetch*` variants directly in the JSX:

```tsx
// app/[locale]/pools/page.tsx (simplified)
export default async function PoolsPage() {
  const result = await safeFetchPools();

  if (!result.ok) {
    return <PoolsErrorState error={result.error} />;
  }
  if (result.value.items.length === 0) {
    return <PoolsEmptyState />;
  }
  return <PoolsTable pools={result.value.items} />;
}
```

## Conventions for the BFF

The BFF lives under `src/app/api/`. Each route handler proxies an endpoint
of `yog-api` for the browser. The conventions are deliberately strict:

- **One BFF route per public `yog-api` endpoint.** If the browser needs
  `/api/pools`, the route handler at `app/api/pools/route.ts` proxies
  `GET ${API_INTERNAL_URL}/api/pools`. The browser never sees `API_INTERNAL_URL`.
- **HTTP error mapping is uniform.**
  - `4xx` → passed through unchanged. The client made a mistake (bad cursor,
    invalid limit) and needs to know.
  - `5xx` → collapsed into `502 Bad Gateway` with a generic body. Internal
    details from `yog-api` (DB errors, stack traces, query plans) must
    never reach the browser.
- **Validation at the boundary.** Inbound query parameters are validated with
  zod before the fetch goes out. A malformed `limit` triggers a `400` from
  the BFF, no upstream call.
- **Server-only secrets.** Anything the browser must not see (auth tokens,
  internal hosts) stays in non-`NEXT_PUBLIC_*` env vars and is read inside
  the route handler.
- **`http-mapping.ts`** — translates `ApiClientError` instances into
  RFC 9457 Problem Details bodies (`{ type, title, status, detail }`)
  served with Content-Type `application/problem+json`. Matches the
  format `yog-api` itself uses for errors, so the dashboard speaks a
  single error dialect across its whole API surface. Exposes
  `problemResponse(body, init)` and the helpers `badRequestProblem`,
  `internalErrorProblem` for local validation failures and unexpected
  errors inside the route handler.
- **`errors.ts`** — `ApiClientError` discriminated union with four
  variants: `timeout`, `network`, `http`, `validation`. The BFF
  internals use this typed surface; the browser-facing wire shape is
  the Problem Details produced by `http-mapping`.

When `yog-api` gains a new endpoint that the dashboard needs, the workflow is:

1. Add a typed `fetchXxx()` / `safeFetchXxx()` in `src/lib/api/<resource>.ts`,
   with a zod schema for the response.
2. Add a BFF route handler in `src/app/api/<resource>/route.ts` that calls it
   and applies the error mapping above.
3. Consume `safeFetchXxx()` from a Server Component (preferred), or call the
   BFF route via `fetch()` from a Client Component when interactivity requires it.

## Error responses (browser-facing)

Every BFF route handler returns errors as RFC 9457 Problem Details,
served with `Content-Type: application/problem+json`. The format
mirrors what `yog-api` returns for its own errors, so the dashboard
parses a single shape regardless of whether the failure originates
in `yog-api` or in the BFF itself.

Wire shape:

    {
      "type": "about:blank",
      "title": "Bad Gateway",
      "status": 502,
      "detail": "upstream API unreachable"
    }

The `title` is the discriminator React branches on for localised
error messages via next-intl. Stable titles in this BFF:

  - "Bad Request"          — local validation or 4xx passthrough from yog-api
  - "Not Found"            — 404 passthrough from yog-api
  - "Bad Gateway"          — yog-api unreachable, returned 5xx, or violated its schema
  - "Gateway Timeout"      — upstream call exceeded the configured timeout
  - "Internal Server Error" — unexpected failure inside the BFF route itself

## Scripts

| Command              | Description                            |
| -------------------- | -------------------------------------- |
| `npm run dev`        | Start the dev server on port 3000      |
| `npm run build`      | Build the standalone production bundle |
| `npm run start`      | Start the built server                 |
| `npm run lint`       | Run ESLint with the Next.js config     |
| `npm run lint:fix`   | Run ESLint and fix what it can         |
| `npm run typecheck`  | Run `tsc --noEmit` against the project |
| `npm test`           | Run the Vitest suite once              |
| `npm run test:watch` | Run Vitest in watch mode               |

## Environment variables

Copy `.env.example` to `.env.local` and fill in the values you need:

```bash
cp .env.example .env.local
```

Variables prefixed with `NEXT_PUBLIC_` are exposed to the browser bundle.
Anything else (including `API_INTERNAL_URL`) is server-only and stays out of
the client bundle.

Notable variables:

| Variable                | Where it's read                                      | Purpose                                                                                                       |
| ----------------------- | ---------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| `API_INTERNAL_URL`      | Server-side only (Server Components, route handlers) | Base URL the BFF uses to reach `yog-api`. In Docker, `http://yog-api:5000`; locally, `http://127.0.0.1:5000`. |
| `NEXT_PUBLIC_APP_URL`   | Both                                                 | Public URL of the app, used for Open Graph metadata and absolute links.                                       |
| `NEXT_PUBLIC_FEATURE_*` | Both                                                 | Feature flags (see below).                                                                                    |

Database credentials must **never** appear in this file — the frontend has no
business knowing about them.

## Feature flags

The dashboard uses a registry-based feature flag system. The single
source of truth is [`src/config/features.ts`](src/config/features.ts):
every flag is declared there with its description, status, and default
value. TypeScript exposes the union of valid flag names through
`FeatureName`, so any reference to an unknown flag fails to compile.

### Toggling a flag

Each flag maps to a `NEXT_PUBLIC_FEATURE_*` environment variable using
the camelCase → SCREAMING_SNAKE_CASE convention. Examples:

```
poolsList    → NEXT_PUBLIC_FEATURE_POOLS_LIST
tvlTotal     → NEXT_PUBLIC_FEATURE_TVL_TOTAL
alertsPanel  → NEXT_PUBLIC_FEATURE_ALERTS_PANEL
```

Only the literal string `true` enables a flag. Any other value
(`1`, `yes`, `True`, empty, unset) keeps it disabled. This strict
parsing avoids silent typo failures.

### Build-time, not runtime

Because Next.js inlines `NEXT_PUBLIC_*` values into the client bundle
at build time, **flipping a flag in production requires a rebuild and
a redeploy**. This is a build-time toggle, not a hot runtime toggle.
A runtime toggle system (DB-backed, modifiable via UI) is on the
roadmap for v0.3 once user accounts and admin areas exist.

### Using a flag in code

```tsx
import { FeatureGate } from "@/components/feature-gate";

<FeatureGate flag="tvlTotal">
  <TvlTotalCard />
</FeatureGate>
```

Or imperatively:

```ts
import { isFeatureEnabled } from "@/config/features";

if (isFeatureEnabled("alertsPanel")) {
  // ...
}
```

## Project layout

```
web/
├── i18n/                            # next-intl routing, request and navigation config
├── messages/                        # locale message bundles (en, fr)
├── public/                          # static assets (favicons, etc.)
├── src/
│   ├── app/
│   │   ├── globals.css
│   │   ├── layout.tsx               # root layout (passthrough)
│   │   ├── [locale]/
│   │   │   ├── layout.tsx           # html/body, intl provider, sidebar
│   │   │   ├── page.tsx             # locale home page
│   │   │   └── pools/page.tsx       # pools listing
│   │   └── api/
│   │       └── pools/route.ts       # BFF route handler — proxies yog-api
│   ├── components/
│   │   ├── feature-gate.tsx         # <FeatureGate flag="..."> wrapper
│   │   └── pools/                   # PoolsTable, PoolsEmptyState, PoolsErrorState, PoolsPagination
│   ├── config/
│   │   ├── features.ts              # feature flag registry + helpers
│   │   └── __tests__/
│   ├── lib/
│   │   ├── api/
│   │   │   ├── client.ts            # fetch wrapper + zod validation
│   │   │   ├── errors.ts            # ApiClientError (discriminated union)
│   │   │   ├── pools.ts             # fetchPools / safeFetchPools
│   │   │   └── tokens.ts            # fetchToken / safeFetchToken
│   │   └── format/
│   │       ├── pubkey.ts            # shortenPubkey
│   │       └── date.ts              # formatRelative / formatAbsolute
│   ├── types/
│   │   └── env.d.ts                 # process.env type augmentation
│   └── proxy.ts                     # locale negotiation (Next 16)
├── Dockerfile
├── eslint.config.mjs
├── next.config.ts
├── package.json
├── postcss.config.mjs
├── tailwind.config.ts
├── tsconfig.json
└── vitest.config.ts
```

## Local development

```bash
npm install
npm run dev
```

Visit <http://localhost:3000>; you will be redirected to `/en` by the
locale proxy. Switch to `/fr` in the URL to see the French version.

The dev server expects `yog-api` to be reachable at the address in
`API_INTERNAL_URL`. The simplest setup is to run the backend stack in
Docker and the frontend natively:

```bash
# From the repo root: backend stack in Docker
docker compose --profile backend up -d

# Back in web/: frontend natively
npm run dev
```

With this setup, `API_INTERNAL_URL=http://localhost:5000` in `.env.local`.

## Docker

A multi-stage Dockerfile produces a minimal production image based on
the Next.js standalone output. It is built and orchestrated as part of
the full stack via `docker compose --profile full up -d` at the repo
root, but can also be built and run standalone:

```bash
docker build -t yog-sothoth-web:dev .
docker run --rm -p 3000:3000 --env-file .env.local yog-sothoth-web:dev
```

Inside the compose network, the container reads `API_INTERNAL_URL=http://yog-api:5000`
— set automatically by `docker-compose.yml`.

## Note on the `proxy.ts` naming

In Next.js 16, the file convention `middleware.ts` was renamed to
`proxy.ts` to clarify that this layer sits at the network boundary and
handles routing concerns rather than Express-style application
middleware. The exported function is also renamed from `middleware`
to `proxy`. next-intl still exposes its helper under
`next-intl/middleware` — only the consumer file name has changed.

## CI

GitHub Actions runs three jobs in parallel on every push and PR that
touches this package — see `.github/workflows/web-quality.yml`:

- **TypeScript** — `npm run typecheck`
- **ESLint** — `npm run lint`
- **Vitest** — `npm test`

A separate workflow (`.github/workflows/web-docker.yml`) builds the
production Docker image and runs a smoke test against the locale
routes. It does not push anywhere — it's a regression guard.

## See also

- [Root README](../README.md) — project pitch, roadmap, getting started
- [`crates/README.md`](../crates/README.md) — Rust workspace architecture, the `yog-api` shape this frontend consumes

## Roadmap

See the [project root](../README.md#roadmap) for the full roadmap. The
v0.1 dashboard (overview + pools pages) is the current focus.