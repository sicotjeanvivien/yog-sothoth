# yog-sothoth-web

Next.js 16 frontend for the Yog-Sothoth liquidity intelligence engine.

This package lives next to the Rust crates of the project but is fully
independent at the Node.js level. It talks to `yog-api` over HTTP only —
it never connects to TimescaleDB, and there is **no BFF layer**: the
browser calls the Rust API directly.

Comments follow the absolute rule of `CLAUDE.md` (*Comments — absolute rule*).

What the web computes stops at the arithmetic over one response and the presentation: a rule
that combines several routes is the API's, rule 11 of `crates/api/README.md`.

## Stack

- **Next.js 16** — App Router, Server Components, standalone output, Turbopack by default
- **React 19.2** — bundled with Next 16
- **TypeScript** — strict mode enabled, including `noUncheckedIndexedAccess`
- **Tailwind CSS v4** — CSS-first: no `tailwind.config.ts`, the theme is declared
  in `src/app/globals.css` and the plugin is wired through PostCSS
- **next-intl 4** — i18n with always-visible locale prefix (`/en/...`, `/fr/...`)
- **zod** — runtime validation of every payload returned by `yog-api`
- **visx** — low-level chart primitives for the dashboard's charts
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
  the signal SSE stream, reconnection refills, the network and
  ingestion poll. It goes through the public gateway (`https://api.yog-scope.xyz`
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
├── schema/zod.gen.ts    # generated: one zod schema and one type per wire
│                        # shape, from crates/api/openapi.json — never edited
├── server/              # one fetcher per resource for Server Components:
│                        # fetchSignals, fetchSwapTotals, fetchTopTokens,
│                        # fetchOpenAnnouncements, … — those a page's
│                        # blocks share go through React's `cache`; a
│                        # `Scope` picks the root or the protocol's route
├── browser/             # browser-side fetchers for Client Component flows
│                        # (signals refill on reconnect, network status)
└── errors.ts            # ApiClientError — discriminated union
```

Every response body is validated with zod before it reaches a
component; a payload that violates the schema is a `validation` error,
not a rendering surprise.

### The API contract

The schemas are **generated**, not written: `npm run gen:api` rewrites
`schema/zod.gen.ts` from `crates/api/openapi.json`, the OpenAPI document the API
derives from its routes ([`crates/api/README.md`](../crates/api/README.md#the-contract)).
The names are the Rust ones — `zSignalResponse` and its type `SignalResponse`,
`EmbeddedTokenResponse` for a token inside a signal — and each route has its
response schema, named after the handler (`zListSignalsResponse`). A fetcher
passes that one to `apiGet`. The values of the query parameters (`PageDir`,
`PagePosition`) are derived from the generated query
schemas, so a value renamed server-side breaks `tsc`. CI's `contract` job fails
when the file is not what the committed document generates.

What the schemas cannot say, and a component must still know:

- **Numbers that stay strings.** A token quantity or a reserve
  (`U128String`) can exceed 2^53: use `BigInt(value)` for arithmetic. A USD
  amount or a rate (`UnsignedDecimal`, `SignedDecimal`) keeps digits a JS
  `number` drops: format it, never compute on `Number(value)`. The `slot` of
  `/api/network` is shown as is.
- **Closed sets fail loudly.** A protocol, a severity, an announcement or event
  kind, a provider or a freshness outside its enum is a `validation` error, never
  an unknown tag reaching the UI.
- **An announcement's `linkUrl` is usually a relative path** (`/changelog#v0.1.0`),
  so it is not a URL; its `severity` is an editorial scale, distinct from a
  signal's even though the tags coincide.
- **A signal's `id`** is the reconciliation key of the live feed after a
  reconnect, and the React key.
- **An error body is read best-effort**: `readRemoteErrorMessage` falls back to
  `null` when it does not match `zProblemDetailsResponse`, leaving the status
  code as the signal. `type` is `about:blank` today; `title` is what
  discriminates.

### The schemas are strict, so the API deploys first

Required fields are required: a schema that tolerates a missing field is how a
field ends up missing for good.

The price is an ordering constraint: **ship the API before the web**. A web
container newer than the API fails validation on every payload whose shape
changed rather than degrading. Today `docker compose --profile full` starts both from
the same commit, so the deployment unit enforces it; the constraint only
becomes real the day the two ship separately.

`ApiClientError` has four kinds, and Server Components catch it and branch on
its kind:

| Kind         | Meaning                                            |
| ------------ | -------------------------------------------------- |
| `timeout`    | the call exceeded the configured timeout           |
| `network`    | fetch failed before an HTTP response existed       |
| `http`       | non-2xx — the RFC 9457 `detail` field is captured as the remote message |
| `validation` | 2xx but the body violated the zod schema           |

```tsx
try {
  page = await fetchTopTokens(period, TOP_TOKENS);
} catch (err) {
  if (err instanceof ApiClientError) {
    return <BlockError title={t("title")} kind={err.details.kind} />;
  }
  throw err;
}
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

Above the feed, **severity × detector filter chips** (`signal-filters.tsx`)
narrow it locally: within a dimension the active values are OR'd, across
dimensions they are AND'd, and an empty selection means "no filter" — the
default state of an alert feed must never hide anything. The filtering
(`filterSignals`, pure and tested) happens at render time over the merged list:
the SSE stream and the 200-item merge are untouched, so a hidden signal stays in
memory and reappears the moment the filter releases it. One ⓘ on the detector
group explains what each detector means.

The **Overview** shows the latest signals as rows of its own, kept live over the
same SSE hook (see *Overview*).

Signals render as cards: token pair (embedded `tokenA`/`tokenB` from the
API, reusing `PoolPairCell`; short pool address while unresolved), a
human-readable per-detector headline phrased from the structured `value`
(i18n, percent-formatted — not the detector's raw English `message`),
and a detail footer (exact value, threshold, raw detector tag). A
detector unknown to the UI falls back to `message`, so a new detector
ships readable before its wording lands here.

## Dashboard frame

The `(dashboard)` layout mounts `DashboardShell`: the sidebar (a sticky rail
from 721 px, the `rail:` breakpoint of `globals.css`; a drawer behind a thin
"Menu" bar up to 720 px), the announcement banner and the live status. The
rail collapses to 64 px from the toggle beside the logo, with a cookie.

**Nothing waits for data** — the layout reads only its two cookies before
rendering: the announcement streams in its own `<Suspense>`, and each Overview
block in its own, behind its skeleton. A failed block shows its error box;
the status code is already sent.

**Menu** — `sidebar/sidebar-nav.ts` is pure data: Overview, then the
platforms (Meteora, with DAMM v2 and DLMM under it; Raydium; Orca), then Token.
An entry is `open` (it has a page and an `href`) or `closed` (tagged "Soon", no
`href`). A platform links to its first open product; with none open it is a
caption over them, and with no product listed it is a closed entry itself.
`SidebarNavList` renders a closed entry as a plain `<span>`, never a link; its
test fails as soon as one becomes a link or loses its tag. To open a top entry
once its page exists, move its key from `ClosedNavKey` to `OpenNavKey`
(`sidebar-keys.ts`) and give it an `href`; a product opens by turning `open`
with its `href`. Every row but a product has its icon in `NAV_ICONS` — a drawn
one, or the platform's logo (`public/logo_<platform>.svg`) — shown in both states;
the compiler refuses a key without one. The collapsed rail keeps every row as
its icon alone, its label in `title`, hides the products, and marks the
platform of the current page. The About page's scope (`PROTOCOL_SCOPE`) is
read from the same data. `/signals` is no longer in the menu.

**Preferences** — at the foot of the menu: the
language, FR | EN (`shared/locale-switch.tsx`, also the site footer's), each a
link to the same page and query in that language. Whether they are open is the
`yog_prefs_open` cookie, read by the layout so the first paint is right. On
the collapsed rail only their icon stays: a click unfolds the rail and opens
them (`preferencesClick`). The folded rail hides an open panel without closing
it; the drawer has no folded form, whatever the collapse cookie says.
`DashboardShell` owns both states.

**Live status** — `live-status/`: `LiveIndicator` polls `/api/ingestion` and
`liveStatusOf` maps it to four states — live (green), delayed (yellow), stale
(grey), offline (light red, the call failed). It sits in the page header (the
Overview only) and opens on click, with the slot and RPC latency of
`/api/network`, and the rows the page streams in — on the Overview, its price
coverage and the hour its figures stop at.

**Fonts** — Geist (text) and Geist Mono (figures, addresses, times) are loaded
by the locale layout, for the dashboard and the site alike; Cinzel too. Their
files live in `src/app/fonts/`, each family next to its OFL licence: the build
fetches nothing, so it cannot fail on Google Fonts. They are Google Fonts' own
files, every subset, fetched once with the user agent Next.js used to request
them (`next/dist/compiled/@next/font/dist/google/fetch-resource.js`); a file
fetched by a recent browser is another build of the font, and moves the line
breaks. The latin subset goes through `next/font/local` (preload, fallback
metrics, the CSS variable); the others are plain `@font-face` rules in
`fonts.css`, each with its `unicode-range`. ⚠️ Every face of a family bears the
name of its const in the layout (`geist`, `geistMono`, `cinzel`): Turbopack
names the CSS variable after the const, and a face under another name is never
used. `__tests__/fonts.test.ts` holds names, ranges and files together.

**Colours** — the dashboard has one colour per role, declared once as
`--color-dash-*` in `globals.css`: `bg`, `surface`, `surface-2`, `rule`,
`ink`, `ink-2`, `ink-3`, `accent`, `up`, `down`, `warn`. Every text colour
reaches at least 4.5:1 on `dash-bg` and `dash-surface`. Violet (`accent`)
marks the brand: the active entry, the keyboard focus. Green says live; green
and red carry a meaning, never the brand. The frame (`shell/`, `sidebar/`,
`live-status/`, `announcements/`) uses these tokens only, with no hex values
and no Tailwind palette. The background is a flat `dash-bg`, which also sits
behind the pages that left the menu, with no glow and no text shadow. The
older palette (`cosmos-*`, `sothoth-*`, `slate-*`) remains in use for
page contents not yet moved to the roles; the Overview's own blocks have
moved, while the ⓘ (`InfoPopover`) and `BlockError` they share keep it.

## Overview

`(dashboard)/overview/page.tsx`, after the canvas: four KPIs (TVL, volume,
fees, active pools), activity by protocol, the latest signals, volume per bar
of the period, the most traded tokens. The header is the title alone, then "Live",
the period and the advanced mode at its right. Every figure comes from yog-analytic's
measures (`/api/swaps/…`, `/api/tvl…`, `/api/pool-creations/…`), which all end at the same `asOf`, the close of the last frozen
hour.

**The period** — `?period=24h|7d|30d` (`lib/overview/period.ts`; missing or
unknown is 24h) drives every label, section title, chart and ⓘ, which speak of
"the chosen period". A bar is an hour over 24h, a 6-hour slice over 7d, a 24-hour
slice over 30d (`SLOT_HOURS`), each ending at `asOf`; over 7d the web sums the
`hourly` series by six, as the API sums its days: whether an hour is covered is the API's. The TVL stays
the value of the last frozen hour; only its change follows the period. The selector (`period-selector.tsx`) is a set of
links, in its own unkeyed `<Suspense>` so a period change keeps it; every block
that reads the period has its `<Suspense key={period}>`, so the change shows
their waiting slots only; the signals do not reload.

**What may not be shown** (`lib/overview/availability.ts`, pure and tested) — a
figure the facts do not cover whole is "—", never a smaller number:
- a period whose window starts before `coveredSince` (the first frozen hour) is
  closed: a button whose panel says when it opens; reached by its URL anyway,
  every block that sums over it shows "—" and the same reason (the TVL stays,
  its change says why it has none). 30d stays closed in the web
  (`CLOSED_PERIODS`) until its times are measured on the prod: its URL falls
  back to 24h, so its reads never run;
- the volume change is "—" when the previous window (`offset`) starts before
  `coveredSince`;
- the TVL change is "—" when `tvl/totals` has no reading a window before the last,
  or no discovered part at it: the hours before the first complete TVL are deleted.
Each "—" carries an ⓘ with its reason.

**ⓘ** — every KPI and section has one, click-toggled
(`shared/info-popover.tsx`), never hover: hover does not exist on touch.

**Blocks** — each fetches on its own, in its own `<Suspense>` with a skeleton
of its final shape and size (`overview-states.tsx`, still when the reader asks
for less motion), and fails on its own: the cause in words and a retry of the
page, or a "—" KPI.

**Late figures** (`lib/overview/staleness.ts`) — a frozen hour normally lags up
to ~70 min. When `asOf` is over two hours back, the computation has stalled: a
band over the page says until when the figures run, each KPI shows that hour,
and the "Live" panel colours it. "Live" itself keeps speaking of ingestion,
which can be fine while the figures stall. The reads several blocks share are fetched
once per render. The volume chart is HTML bars (`volume-bars.tsx`): two dashed
marks on round values (`roundTicks`), a fixed reading line above, the bar under
the pointer, finger or arrow keys full and the others faded. A volume, of a
token, a protocol or a KPI, that covers only part of its pool-hours carries the
coverage mark (`volumeCoverage`): the share valued, floored to a whole percent so
a partial coverage never reads "100 %".

**Advanced mode** — denser, not longer: six KPIs instead of four (swaps and
their rate per second, turnover — the period's volume over the TVL — each with
its ⓘ), the fees and active pools of each protocol, and the fees by hour or by
day under the volume; cells, table rows and headings tighten, and the order of
the sections never changes. The server renders both modes; `OverviewMode` puts
`data-overview-advanced` over the page and the `adv:` variant (`globals.css`)
shows, hides and tightens, so switching fetches nothing. The switch writes the
`yog_overview_advanced` cookie (one year, declared in `/privacy`), which the
page reads for the first paint. Each block's `MIN_HEIGHT` has its advanced
value, so its waiting slot keeps its size in both modes.

## Protocol page

`(dashboard)/protocols/meteora-damm-v2/page.tsx`, after the canvas: a header,
the protocol's KPIs, then the row of its tabs and the content of the one
chosen.

**Header** — `protocol/protocol-header.tsx`: the platform, then the title, its ⓘ
giving the program and since when the figures are measured (`coveredSince`,
the first frozen hour), and at its right "Live", the period and the advanced
mode: `overview/header-controls.tsx`, shared with the Overview, takes the
page's path, the query it keeps and the scope its "Live" rows and its periods are read in.

**KPIs** — the Overview's cells take a `scope` (`lib/api/server/scope.ts`): `"all"`
on the Overview, which reads the root routes and adds up their protocols, the protocol
here, which reads its own routes (`/api/protocols/{protocol}/…`) — never a root route
filtered in the page. The page adds its fees with their
effective rate and, in the advanced mode, the protocol's fees with their share
(`protocol/protocol-kpis.tsx`). The advanced mode is one choice for the whole
dashboard: same `OverviewMode`, same cookie.

**01 Liquidity** — `protocol/liquidity/liquidity-tab.tsx`: the period's
created, added, removed and net — a pool's creation deposits its first
liquidity without an addition, so it has its own colour (`dash-flow-create`),
stacked under the additions; the TVL hour by hour on a tight axis
(`lib/protocol/liquidity.ts`), its reading fixed above and a dashed mark under
the pointer; the liquidity created and added above the axis and removed under
it (`dash-flow-out`, darker than the text red), per bar
of the period, with symmetric marks; and, in the advanced mode, the largest moves
beside the chart, so the page is no longer. The liquidity is measured from a
later hour than the swaps: until a window is covered, its totals, bars and
moves say so instead of a partial sum, while the period stays open on the swaps' span. Opening this tab opened the menu entry,
with a green dot beside it while the ingestion is live
(`sidebar/product-live-dot.tsx`), and put the page in the sitemap.

**02 Volume** — `protocol/volume/volume-tab.tsx`: where the volume comes from,
each bar of the protocol's swaps split between its rotation runs
(`dash-run`) and the rest (`dash-run-out`), as `rotation-runs/hourly|daily` serves
it; `lib/protocol/volume.ts` sums the hours of a 7-day slice, split only when each
of them is. The runs are measured from a later hour than the swaps: a bar they do
not split keeps its volume, unsplit, in a neutral tint, and the legend says since
which hour they split it. Under the chart, the
period's detail, replaced by the bar's under the pointer
(`protocol/split-bars.tsx`, shared with the fees). Beside it, the runs of the period
(`rotation-runs/totals`: their count and pools, those in progress over 24 h,
the median and p90 length, the share seen in the first hour), and under both
the five largest (`rotation-runs?sort=volume`), named by their tokens. The
runs' totals and list end at the last tranche written: until a window is
covered, they say so instead of a partial count. The words are "rafale" and
"rotation run": what is seen, never an intent.

**03 Fees** — `protocol/fees/fees-tab.tsx`: where the protocol's fees go, the
LPs' part (`claiming_fee + compounding_fee`), the protocol's and the referral's,
read on `swaps/hourly|daily` of the protocol and summed per bar by
`lib/protocol/fees.ts`: per destination over the period, amount and share, and
per bar stacked in the three tints (`dash-fee-*`), the bar under the pointer
detailed under the chart. The three parts make the fees whole. Beside it, what the
LPs take back: the fees and the rewards claimed, valued, with their addresses
(`position-fee-claims/totals`, `reward-claims/totals`), and nothing else: a share of the
LPs' fees claimed would cross two routes, a rule the web does not hold. The claims are measured from a later hour
than the swaps: until a window is covered, the block says so. The LPs' split between
claiming and compounding is not drawn yet.

**04 Pools & positions** — `protocol/pools/pools-tab.tsx`: the pools born over the
period, bar by bar, from `pool-creations/hourly|daily` of the protocol
(`lib/protocol/pools.ts`), and the five whose first deposit is worth the most, from
`liquidity-moves?kind=create&sort=amount`: pair, creator, lock, deposit, age; a lock not measured
yet reads "—", apart from "none" (`lockKey`). A pool whose deposit
could not be valued is not in that list; the chart counts it. In the advanced mode, beside them
so the page is no longer, the positions' life cycle in two fixed columns, 24 h and 7 d, whatever
the period: `position-changes/totals` for the positions opened, locked, split and closed, and the
`owners` of `liquidity-moves/totals` for the distinct LPs; a window without a row reads "—"
(`lifeCountOf`).

**Tabs** — `lib/protocol/tabs.ts` lists the six tabs of the canvas, in order;
a tab joins the row when its content ships (`READY_TABS`), numbered by its
place in the full list so it never moves. `?tab=` picks one, a bad value falls
back to the first ready, which, like the default period, the URL leaves out
(`pageQuery`, `tabQuery`); tab links keep the period, period links keep the
tab, and a digit key opens its tab, Shift allowed for AZERTY. The KPIs'
`<Suspense>` is keyed by the period only: a tab change re-reads them on the
server and keeps them on screen.

## Site

The `(marketing)` layout wraps every page in `site-root`: the dashboard's
`dash-*` tokens and fonts, a flat `dash-bg`, the same keyboard focus. Its
header (`navbar/`) folds its links behind "Menu" up to 720 px (`rail:`), the
short "Dashboard" button staying out; its grids (`site-grid.ts`) drop columns
up to 1100 px (`wide:`) and their column gap up to 720 px. The footer ends on
the version (the first entry of `changelog/releases.ts`) and the FR | EN
switch, which leads to the same page in the other language. The logo is
`public/logo_yog_scope.svg`, in the accent colour, through
`shared/logo-mark.tsx`, on the site and in the dashboard menu.

The home page (`home/`) is static and calls no API: its dashboard preview is
an illustration with fixed figures, and its facts strip is plain text, to
re-check against the prod when a fact changes. Its text stays in Geist's
`latin` subset: one character outside it (a superscript `ᵉʳ`) makes the
browser fetch another 17 KB font file.

The About page (`about/`) is static too. Its scope lists the protocol group
of the dashboard menu (`dashboard/sidebar/sidebar-nav.ts`), each one observed
or "Soon"; its facts are re-checked against the prod like the home page's.

The support page (`support-us/`) offers a donation in SOL only.

The three legal pages share one template (`legal/legal-document.tsx`): tabs
between them, a table of contents, numbered sections. Each page builds its
sections from its own bundle (`privacy.json`, `legal-notice.json`,
`terms.json`); the template's own words live under `Legal`.

The changelog's source is `changelog/releases.ts`, written in the PR that
ships each release, in English only. Every line is marked `technical` or not:
a technical line is folded under "Technical details" and left out of the
filters, and a release with no visible line reads "Internal improvements
only." in place of its summary. The page is server-rendered whole; the
filter (the only client island) sets `data-filter` on its frame, and CSS
hides the rest.

### Search engines and link previews

`src/config/site.ts` holds the site's origin, the preview image
(`public/og-image.png`, one for every page and both languages) and the pages in
two lists: `SITEMAP_PATHS`, which `app/sitemap.ts` lists and `app/robots.ts`
points at, the Overview included, and `UNLISTED_PATHS`, the signal feed, served
with its metadata but kept out of the sitemap — its reads are live, slow, and
fail with the API. The locale layout sets `metadataBase`
and the only title template, `%s — ${SITE_NAME}`. Each page builds its metadata
through `lib/seo/page-metadata.ts` — title, description, preview, canonical
without parameters, its two languages and `x-default` to the unprefixed path —
because Next merges metadata shallowly: a page setting `title` alone would keep
the layout's `og:title`.

A new page goes in one of the two lists and calls `pageMetadata`, or
`lib/seo/__tests__/seo.test.ts` fails. Cloudflare prepends its own comments to
`robots.txt`: check the `Sitemap:` line on the prod, after it.

## Operator announcements

The dashboard layout (Server Component) fetches `GET
/api/announcements?status=open` on every dashboard render, reads the dismiss
cookie, and picks the one announcement to show (`pickAnnouncement`,
`lib/announcements/` — the most severe non-dismissed one, then the most
recent, the API's order). The pick happens server-side
so the first paint is already correct — the `sidebar-state` cookie
pattern. The client island (`AnnouncementBanner`) only owns the dismiss
click: it appends the id to the single CSV cookie
(`yog_announcements_dismissed`, capped, 90-day max-age) and hides
itself. A failing announcements call degrades to "no banner" — it never
takes the dashboard down.

The banner's severity styling is its own mapping (an announcement
severity is an editorial choice, not a detector conclusion), onto the
dashboard's role colours: neutral for info, `dash-warn`, `dash-down`.

## Scripts

| Command              | Description                            |
| -------------------- | -------------------------------------- |
| `npm run dev`        | Start the dev server on port 3000      |
| `npm run build`      | Build the standalone production bundle |
| `npm run start`      | Start the built server                 |
| `npm run lint`       | Run ESLint with the Next.js config     |
| `npm run lint:fix`   | Run ESLint and fix what it can         |
| `npm run typecheck`  | Run `tsc --noEmit` against the project |
| `npm run gen:api`    | Regenerate `src/lib/api/schema/` from `crates/api/openapi.json` |
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
transactionFeed → NEXT_PUBLIC_FEATURE_TRANSACTION_FEED
tvlTotal        → NEXT_PUBLIC_FEATURE_TVL_TOTAL
alertsPanel     → NEXT_PUBLIC_FEATURE_ALERTS_PANEL
```

Only the literal string `true` enables a flag. Any other value
(`1`, `yes`, `True`, empty, unset) keeps it disabled. This strict
parsing avoids silent typo failures.

### Build-time, not runtime

Because Next.js inlines `NEXT_PUBLIC_*` values into the client bundle
at build time, **flipping a flag in production requires a rebuild and
a redeploy**. This is a build-time toggle, not a hot runtime toggle.
A runtime toggle system (DB-backed, modifiable via UI) only makes sense
once user accounts and admin areas exist.

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
│   │   ├── fonts/                   # .woff2 per family and subset, OFL licences, fonts.css
│   │   ├── layout.tsx               # root layout (passthrough)
│   │   └── [locale]/
│   │       ├── layout.tsx           # html/body, intl provider, fonts
│   │       ├── (dashboard)/         # app shell: menu, network status
│   │       │   ├── overview/        # KPIs, activity by protocol, signals, volume, tokens
│   │       │   └── signals/         # live signal feed (SSR + SSE)
│   │       └── (marketing)/         # public pages: home, about, terms,
│   │                                # privacy, legal-notice, support-us,
│   │                                # changelog
│   ├── components/
│   │   ├── feature-gate.tsx         # <FeatureGate flag="..."> wrapper
│   │   ├── dashboard/               # per-page sections + shell, sidebar,
│   │   │                            # signals (SignalFeed, useSignalStream),
│   │   │                            # announcements
│   │   ├── marketing/               # navbar, footer, per-page sections
│   │   └── shared/                  # buttons, icons, popovers, …
│   ├── config/
│   │   └── features.ts              # feature flag registry + helpers
│   ├── lib/
│   │   ├── api/                     # API layer (see above)
│   │   ├── announcements/           # pickAnnouncement + dismiss cookie
│   │   ├── config/                  # zod-validated env (server + client)
│   │   ├── coverage/                # volumeCoverage (pure, tested)
│   │   ├── overview/                # period, availability, chart, totals (pure, tested)
│   │   ├── format/                  # pubkey, date, number formatters
│   │   └── signals/                 # mergeSignals, filterSignals (pure, tested)
│   ├── types/
│   │   └── env.d.ts                 # process.env type augmentation
│   └── proxy.ts                     # locale negotiation (Next 16)
├── Dockerfile
├── audit-exceptions.json            # accepted npm advisories, each with its reason
├── audit.sh                         # the CI's npm audit, runnable locally
├── eslint.config.mjs
├── next.config.ts
├── package.json
├── postcss.config.mjs               # @tailwindcss/postcss — v4 has no JS config,
│                                    # the theme lives in src/app/globals.css
├── tsconfig.json
└── vitest.config.ts
```

## Local development

```bash
npm install
npm run dev
```

npm itself must be 12.x: `devEngines` in `package.json` makes any other
major refuse to run here (`EBADDEVENGINES`), in CI and in the Docker image
as on your machine. Node 26 ships an older one — `npm install -g npm@12`.
The reason is the lockfile: two majors write it differently, and a lock one
accepts the other can reject.

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
docker build --build-arg NEXT_PUBLIC_YOG_API_URL=http://localhost:5000 \
  -t yog-sothoth-web:dev .
docker run --rm -p 3000:3000 --add-host=host.docker.internal:host-gateway \
  -e YOG_API_INTERNAL_URL=http://host.docker.internal:5000 yog-sothoth-web:dev
```

The two URLs arrive at different moments, and that is not a choice:

- `NEXT_PUBLIC_YOG_API_URL` is a **build argument**. `next build` writes it
  into the client bundle as a literal and nothing reads it afterwards, so
  passing it to `docker run` changes nothing. The build stops if it is
  empty.
- `YOG_API_INTERNAL_URL` is read by the server **at run time**.

`docker-compose.yml` passes both — the build argument defaults to
`http://localhost:5000`. The production image is built by the `images`
workflow from `docker/docker-bake.hcl`, which derives it from the repository
variable `YOG_API_DOMAIN` (`docker/README.md`, *The registry*).
`src/lib/config/__tests__/compose-env.test.ts`
runs the server schema on the compose's values, and checks every build
argument the compose passes is declared in the Dockerfile.

## Security headers

Every response carries `X-Content-Type-Options: nosniff`,
`Referrer-Policy: strict-origin-when-cross-origin` and a Content Security
Policy, defined once in `src/lib/config/security-headers.ts` and applied by
`headers()` in `next.config.ts`; `X-Powered-By` is turned off.

The CSP is `frame-ancestors 'none'; object-src 'none'; base-uri 'self';
connect-src 'self' <API origin>`. Two things it leaves out on purpose:

- **no script policy** — Next.js inlines its own bootstrap scripts, so one
  that holds needs a per-request nonce, and with it every page rendered
  through `proxy.ts` instead of served pre-rendered;
- **no `img-src`** — token logos come from arbitrary registry hosts.

The API origin in `connect-src` comes from `NEXT_PUBLIC_YOG_API_URL`, so,
like the client bundle, it is **fixed at build time**: an image built for one
API origin blocks every other. HSTS is set by the TLS edge,
the server's entry proxy, not here.

## Note on the `proxy.ts` naming

In Next.js 16, the file convention `middleware.ts` was renamed to
`proxy.ts` to clarify that this layer sits at the network boundary and
handles routing concerns rather than Express-style application
middleware. The exported function is also renamed from `middleware`
to `proxy`. next-intl still exposes its helper under
`next-intl/middleware` — only the consumer file name has changed.

## CI

GitHub Actions runs four jobs in parallel on every push and PR that
touches this package — see `.github/workflows/web-quality.yml`. The workflow
itself starts on every PR, so that its `Verdict (web-quality)` check exists for
the ruleset of `main` to require; the jobs are skipped when nothing here moved
(`crates/README.md`, *CI*):

- **TypeScript** — `npm run typecheck`
- **ESLint** — `npm run lint`
- **Vitest** — `npm test`
- **npm audit** — `bash audit.sh`: fails on a known advisory, moderate or
  above, against a runtime dependency. Dev dependencies are left out on
  purpose: the image ships the standalone output, not `node_modules`. An
  advisory can only be accepted through `audit-exceptions.json`, with a
  written `reason` and a `revisit` condition (format in the script's header),
  in the spirit of `.cargo/audit.toml`.

A separate workflow (`.github/workflows/web-docker.yml`) builds the
production Docker image and runs a smoke test against the locale
routes, with its own `Verdict (web-docker)`. It does not push anywhere —
it's a regression guard.

## See also

- [Root README](../README.md) — project pitch, status, getting started
- [`crates/README.md`](../crates/README.md) — Rust workspace architecture, the `yog-api` shape this frontend consumes
