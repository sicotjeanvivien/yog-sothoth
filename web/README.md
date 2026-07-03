# yog-sothoth-web

Next.js 16 frontend for the Yog-Sothoth liquidity intelligence engine.

This package lives next to the Rust crates of the project but is fully
independent at the Node.js level. It talks to `yog-api` over HTTP only —
it never connects to TimescaleDB, and there is **no BFF layer**: the
browser calls the Rust API directly.

## Stack

- **Next.js 16** — App Router, Server Components, standalone output, Turbopack by default
- **React 19.2** — bundled with Next 16
- **TypeScript** — strict mode enabled, including `noUncheckedIndexedAccess`
- **Tailwind CSS** — palette extracted from the Yog-Sothoth mockups
- **next-intl 4** — i18n with always-visible locale prefix (`/en/...`, `/fr/...`)
- **zod** — runtime validation of every payload returned by `yog-api`
- **visx** — low-level chart primitives for the pool time-series charts
- **Vitest** — unit tests in Node environment

## Architecture

```
┌──────────────────────┐   HTTP (SSR render)    ┌─────────┐
│  Next.js server      │───────────────────────▶│         │
│  (Server Components) │  YOG_API_INTERNAL_URL  │ yog-api │
└──────────────────────┘                        │ (Rust)  │
┌──────────────────────┐   HTTP + SSE           │         │
│  Browser             │───────────────────────▶│         │
│  (Client Components) │  NEXT_PUBLIC_YOG_API…  └─────────┘
└──────────────────────┘   CORS-locked
```

`yog-api` has two consumers in this package:

- **Server Components** render the initial page data on the Node.js
  server. They reach the API over the internal network via
  `YOG_API_INTERNAL_URL` (`http://yog-api:5000` inside Docker).
- **The browser** calls the API directly for everything dynamic —
  the signal SSE stream, reconnection refills, the network-status
  poll. It goes through the public gateway (`https://api.yog-scope.xyz`
  in production, `http://localhost:5000` in dev) via
  `NEXT_PUBLIC_YOG_API_URL`. `yog-api`'s CORS layer authorises the
  dashboard origin.

There used to be BFF route handlers under `app/api/` proxying the API
for the browser. They were removed: pure proxies with no added value —
their removal collapses one network hop, one error format, and one set
of duplicated validations. Both runtimes share the same client core, so
behaviour stays identical on either path.

## The API layer (`src/lib/api/`)

```
lib/api/
├── client/
│   ├── client-core.ts   # runtime-agnostic core: URL building, timeout +
│   │                    # AbortController, error classification, RFC 9457
│   │                    # envelope parsing, zod validation
│   ├── server.ts        # apiGet()        — reads YOG_API_INTERNAL_URL
│   └── browser.ts       # apiGetBrowser() — reads NEXT_PUBLIC_YOG_API_URL
├── schema/              # one zod schema per resource (pool, signal, stats,
│                        # swap-event, …) + shared primitives (BigDecimal,
│                        # SignedBigDecimal, page envelope, RFC 9457 body)
├── server/              # one fetcher per resource for Server Components:
│                        # fetchPools, fetchPool, fetchStats, fetchSignals,
│                        # fetchPoolHistory, fetchTopPools, …
├── browser/             # browser-side fetchers for Client Component flows
│                        # (signals refill on reconnect, network status)
├── type/                # pagination types shared by fetchers
├── errors.ts            # ApiClientError — discriminated union
└── safe-fetch.ts        # safeFetch() — Result-like wrapper for components
```

Every response body is validated with zod before it reaches a
component; a payload that violates the schema is a `validation` error,
not a rendering surprise.

`ApiClientError` has four kinds, and Server Components branch on them
through `safeFetch` instead of try/catch:

| Kind         | Meaning                                            |
| ------------ | -------------------------------------------------- |
| `timeout`    | the call exceeded the configured timeout           |
| `network`    | fetch failed before an HTTP response existed       |
| `http`       | non-2xx — the RFC 9457 `detail` field is captured as the remote message |
| `validation` | 2xx but the body violated the zod schema           |

```tsx
const outcome = await safeFetch(() => fetchPools());
if (outcome.kind === "error") {
  return <PageError reason={outcome.reason} />;
}
return <PoolsTable pools={outcome.data.items} />;
```

Anything that is *not* an `ApiClientError` is re-thrown so it surfaces
in the Next.js error boundary instead of being collapsed into a
block-level error state.

## Live signal feed

The `/signals` dashboard page combines both consumers:

- the first page of signals is fetched server-side (`fetchSignals`) and
  rendered by the Server Component;
- the `useSignalStream` hook (`components/dashboard/signals/`) then
  opens an `EventSource` directly on `GET /api/signals/stream` (SSE).
  Each event is zod-parsed (malformed → warn + skip) and merged through
  the pure `mergeSignals` helper (`lib/signals/`): dedup by id, sort by
  `(triggeredAt, id)` descending, cap at 200 rows. On reconnection the
  hook refetches page 1 from the browser and reconciles by id, so a
  connection gap never leaves holes in the feed.

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

Both env surfaces are **validated with zod at load time** — a missing or
malformed value fails fast (mirrors `ConfigError::InvalidValue` in the
Rust services). Server vars live in `lib/config/server-env.schema.ts`,
browser vars in `lib/config/client-env.schema.ts`; only `NEXT_PUBLIC_*`
values ever reach the client bundle.

| Variable                          | Surface     | Purpose                                                                                     |
| --------------------------------- | ----------- | ------------------------------------------------------------------------------------------- |
| `YOG_API_INTERNAL_URL`            | server only | Base URL for SSR calls to `yog-api`. In Docker, `http://yog-api:5000`; natively, `http://localhost:5000`. No trailing slash. |
| `YOG_API_TIMEOUT_MS`              | server only | Timeout for SSR → `yog-api` calls.                                                          |
| `NEXT_PUBLIC_YOG_API_URL`         | browser     | Public gateway URL the browser calls directly (`https://api.yog-scope.xyz` in production).  |
| `NEXT_PUBLIC_YOG_API_TIMEOUT_MS`  | browser     | Timeout for browser → `yog-api` calls.                                                     |
| `NEXT_PUBLIC_FEATURE_*`           | browser     | Feature flags (see below).                                                                  |

Database credentials must **never** appear in this file — the frontend
has no business knowing about them.

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
A runtime toggle system (DB-backed, modifiable via UI) only makes sense
once user accounts and admin areas exist (v0.2).

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
├── messages/                        # locale message bundles (en/, fr/)
├── public/                          # static assets (favicons, etc.)
├── src/
│   ├── app/
│   │   ├── globals.css
│   │   ├── layout.tsx               # root layout (passthrough)
│   │   └── [locale]/
│   │       ├── layout.tsx           # html/body, intl provider
│   │       ├── (dashboard)/         # app shell: sidebar + network status
│   │       │   ├── overview/        # global KPIs + top pools
│   │       │   ├── pools/           # pools listing (cursor pagination)
│   │       │   ├── pools/[address]/ # pool detail: state, fees, charts
│   │       │   └── signals/         # live signal feed (SSR + SSE)
│   │       └── (marketing)/         # public pages: home, about, terms,
│   │                                # privacy, legal-notice, support-us
│   ├── components/
│   │   ├── feature-gate.tsx         # <FeatureGate flag="..."> wrapper
│   │   ├── dashboard/               # per-page sections + shell, sidebar,
│   │   │                            # signals (SignalFeed, useSignalStream),
│   │   │                            # pool-detail/charts (visx)
│   │   ├── marketing/               # navbar, footer, per-page sections
│   │   └── shared/                  # pagination, buttons, icons, …
│   ├── config/
│   │   └── features.ts              # feature flag registry + helpers
│   ├── lib/
│   │   ├── api/                     # API layer (see above)
│   │   ├── config/                  # zod-validated env (server + client)
│   │   ├── format/                  # pubkey, date, number formatters
│   │   └── signals/                 # mergeSignals (pure, tested)
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
`YOG_API_INTERNAL_URL` — and the browser at the address in
`NEXT_PUBLIC_YOG_API_URL`. The simplest setup is to run the backend
stack in Docker and the frontend natively:

```bash
# From the repo root: backend stack in Docker
docker compose --profile backend up -d

# Back in web/: frontend natively
npm run dev
```

With this setup, both URLs point to the same place in `.env.local`:

```
YOG_API_INTERNAL_URL=http://localhost:5000
NEXT_PUBLIC_YOG_API_URL=http://localhost:5000
```

## Docker

A multi-stage Dockerfile produces a minimal production image based on
the Next.js standalone output. It is built and orchestrated as part of
the full stack via `docker compose --profile full up -d` at the repo
root, but can also be built and run standalone:

```bash
docker build -t yog-sothoth-web:dev .
docker run --rm -p 3000:3000 --env-file .env.local yog-sothoth-web:dev
```

Inside the compose network, the container reads
`YOG_API_INTERNAL_URL=http://yog-api:5000` — set automatically by
`docker-compose.yml`.

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
