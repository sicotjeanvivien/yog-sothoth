# web/

Next.js application — dashboard UI and backend API for yog-sothoth.

This is a single Next.js process that covers both the frontend and the backend.
No separate Node.js server — API Routes handle all data access.

---

## Structure

```
web/
├── app/
│   ├── api/                  ← API Routes (backend, runs server-side)
│   │   ├── pools/            ← pool registry: list, add, remove watched pools
│   │   ├── metrics/          ← time-series metrics per pool
│   │   └── alerts/           ← alert configuration and history
│   └── (dashboard)/          ← UI pages and components
├── public/
│   └── wasm/                 ← compiled WASM module (output of wasm-pack)
└── lib/
    └── wasm.ts               ← WASM loader and typed bindings
```

---

## API Routes

| Endpoint | Method | Description |
|---|---|---|
| `/api/pools` | GET | List all watched pools |
| `/api/pools` | POST | Add a pool to watch |
| `/api/pools/[address]` | DELETE | Remove a pool |
| `/api/metrics/[address]` | GET | Time-series metrics for a pool |
| `/api/alerts` | GET | List configured alerts |
| `/api/alerts` | POST | Create an alert |
| `/api/alerts/[id]` | DELETE | Remove an alert |

All routes read from TimescaleDB. The indexer writes, Next.js reads — no direct communication between the two processes.

---

## WASM integration

AMM calculations (price, slippage, imbalance) run client-side via the `yog-core` WASM module.

The WASM module is built from `crates/wasm/` and placed in `public/wasm/`.
It is loaded asynchronously on first use via `lib/wasm.ts`.

```ts
import { loadWasm } from '@/lib/wasm'

const wasm = await loadWasm()
const price = wasm.current_price(reserve_a, reserve_b)
```

---

## Real-time updates

The dashboard receives live metrics via WebSocket — no polling.

The Next.js server maintains a WebSocket connection to the indexer's notification channel (via TimescaleDB LISTEN/NOTIFY or a dedicated pub/sub) and pushes updates to the browser.

---

## Environment variables

Create a `.env.local` file at the root of `web/`:

```env
DATABASE_URL=postgresql://yog:yog@localhost:5433/yog_sothoth
```

---

## Getting started

```bash
cd web
npm install
npm run dev
```

The dashboard is available at [http://localhost:3000](http://localhost:3000).

---

## Commands

```bash
npm run dev       # development server with hot reload
npm run build     # production build
npm run start     # start production server
npm run lint      # ESLint
npm run typecheck # TypeScript type checking
```

---

## Prerequisites

The WASM module must be built before running the frontend:

```bash
# From the repo root
wasm-pack build crates/wasm --target web --out-dir web/public/wasm
```

TimescaleDB must be running:

```bash
# From the repo root
docker compose up -d
```
