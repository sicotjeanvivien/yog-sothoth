# yog-sothoth-web

Next.js 16 frontend for the Yog-Sothoth liquidity intelligence engine.

This is an npm workspace package living next to the Rust crates of the
project. It is fully independent at the Node.js level and only talks to
the same TimescaleDB instance that the indexer writes to (read-only
user, see Milestone 0.2).

## Stack

- **Next.js 16** — App Router, Server Components, standalone output, Turbopack by default
- **React 19.2** — bundled with Next 16
- **TypeScript** — strict mode enabled, including `noUncheckedIndexedAccess`
- **Tailwind CSS** — palette extracted from the Yog-Sothoth mockups
- **next-intl 4** — i18n with always-visible locale prefix (`/en/...`, `/fr/...`)
- **postgres.js** — Postgres driver with tagged-template SQL and a singleton pool
- **Zod** — runtime validation of database rows
- **Vitest** — unit tests in Node environment

## Scripts

| Command              | Description                              |
| -------------------- | ---------------------------------------- |
| `npm run dev`        | Start the dev server on port 3000        |
| `npm run build`      | Build the standalone production bundle   |
| `npm run start`      | Start the built server                   |
| `npm run lint`       | Run ESLint with the Next.js config       |
| `npm run lint:fix`   | Run ESLint and fix what it can           |
| `npm run typecheck`  | Run `tsc --noEmit` against the project   |
| `npm test`           | Run the Vitest suite once                |
| `npm run test:watch` | Run Vitest in watch mode                 |

## Environment variables

Copy `.env.example` to `.env.local` and fill in the values you need:

```bash
cp .env.example .env.local
```

Variables prefixed with `NEXT_PUBLIC_` are exposed to the browser bundle.
Database credentials must **never** carry that prefix.

## Database

The web app connects to the same TimescaleDB instance the indexer
writes to. It uses the read-only `yog_web` role provisioned by
[`crates/indexer/migrations/002_web_readonly_user.sql`](../crates/indexer/migrations/002_web_readonly_user.sql).

### Connection pool

A single `postgres.js` client is shared across the entire Node
process via a `globalThis` singleton. The singleton survives Next.js
hot reloads in development and reuses module-cached state in
production. See [`src/lib/db/client.ts`](src/lib/db/client.ts).

### Repository layer

Each table the web app reads from has its own repository module
under [`src/lib/repositories/`](src/lib/repositories/). Repositories
own the SQL string, validate every row through a Zod schema, and
expose a camelCase domain shape. Route Handlers consume that shape
and never see raw rows.

### Error handling

All database failures are wrapped in `DatabaseError` with one of
four kinds:

- `connection` — driver could not reach Postgres → API responds 503
- `query` — SQL ran but returned an error → API responds 500
- `validation` — row shape diverged from the schema → API responds 500
- `unknown` — anything else → API responds 500

A 503 keeps the rest of the app running so a transient database
outage does not break the locale proxy or the home page.

### Endpoints (v0.1)

| Method | Path         | Description                          |
| ------ | ------------ | ------------------------------------ |
| GET    | `/api/pools` | List every pool observed by the indexer |

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
│   │   ├── layout.tsx               # required root layout (passthrough)
│   │   ├── [locale]/
│   │   │   ├── layout.tsx           # html/body, intl provider
│   │   │   └── page.tsx             # locale home page
│   │   └── api/
│   │       └── pools/
│   │           └── route.ts         # GET /api/pools
│   ├── components/
│   │   └── feature-gate.tsx         # <FeatureGate flag="..."> wrapper
│   ├── config/
│   │   ├── features.ts              # feature flag registry + helpers
│   │   └── __tests__/
│   ├── lib/
│   │   ├── db/
│   │   │   ├── client.ts            # postgres.js singleton
│   │   │   └── errors.ts            # DatabaseError type
│   │   └── repositories/
│   │       ├── pools.ts             # listPools()
│   │       └── schemas.ts           # Zod schemas + toPool mapper
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

## Docker

A multi-stage Dockerfile produces a minimal production image based on
the Next.js standalone output:

```bash
docker build -t yog-sothoth-web:dev .
docker run --rm -p 3000:3000 --env-file .env.local yog-sothoth-web:dev
```

## Note on the `proxy.ts` naming

In Next.js 16, the file convention `middleware.ts` was renamed to
`proxy.ts` to clarify that this layer sits at the network boundary and
handles routing concerns rather than Express-style application
middleware. The exported function is also renamed from `middleware`
to `proxy`. next-intl still exposes its helper under
`next-intl/middleware` — only the consumer file name has changed.

## Roadmap

This package was bootstrapped during **Milestone 0** of the v0.1
roadmap. Subsequent milestones add the dashboard skeleton with
feature flags (Milestone 2) and polish (Milestone 3).

See the project root for the full roadmap.