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

## Scripts

| Command            | Description                              |
| ------------------ | ---------------------------------------- |
| `npm run dev`      | Start the dev server on port 3000        |
| `npm run build`    | Build the standalone production bundle   |
| `npm run start`    | Start the built server                   |
| `npm run lint`     | Run ESLint with the Next.js config       |
| `npm run typecheck`| Run `tsc --noEmit` against the project   |

Note: Turbopack is the default bundler in Next 16, no `--turbopack`
flag is required anymore.

## Environment variables

Copy `.env.example` to `.env.local` and fill in the values you need:

```bash
cp .env.example .env.local
```

Variables prefixed with `NEXT_PUBLIC_` are exposed to the browser bundle.
Database credentials must **never** carry that prefix.

## Project layout

```
web/
├── i18n/                # next-intl routing, request and navigation config
├── messages/            # locale message bundles (en, fr)
├── public/              # static assets (favicons, etc.)
├── src/
│   ├── app/
│   │   ├── globals.css
│   │   ├── layout.tsx           # required root layout (passthrough)
│   │   └── [locale]/
│   │       ├── layout.tsx       # html/body, intl provider
│   │       └── page.tsx         # locale home page
│   └── proxy.ts                 # locale negotiation (was middleware.ts in Next 15)
├── Dockerfile
├── next.config.ts
├── package.json
├── postcss.config.mjs
├── tailwind.config.ts
└── tsconfig.json
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
roadmap. Subsequent milestones add the database layer (Milestone 1),
the dashboard skeleton with feature flags (Milestone 2), and polish
(Milestone 3).

See the project root for the full roadmap.