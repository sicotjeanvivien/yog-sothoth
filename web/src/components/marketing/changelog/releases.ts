/**
 * The changelog, one entry per release, edited at each release; its test guards unique
 * `vX.Y.Z` versions, newest first. English only. ⚠️ `version` is the anchor id an announcement's
 * `link_url` points at (`/changelog#<version>`).
 */

export type ReleaseSectionKind = "features" | "fixes";

export type ReleaseItem = {
  /** Free English copy, one bullet. */
  text: string;
  /** Only the code is concerned: folded under the technical details, out of the filters. */
  technical: boolean;
};

export type ReleaseSection = {
  /** Which section label the chrome shows (i18n key). */
  kind: ReleaseSectionKind;
  items: readonly ReleaseItem[];
};

export type Release = {
  /** `vX.Y.Z` — display name AND anchor id of the block. */
  version: string;
  /** ISO date (YYYY-MM-DD), rendered localized. */
  date: string;
  /** One-sentence headline under the version. */
  summary: string;
  sections: readonly ReleaseSection[];
};

/** Newest first — the order the page renders. */
export const RELEASES: readonly Release[] = [
  {
    version: "v0.1.7",
    date: "2026-10-17",
    summary:
      "The dashboard frame no longer waits for its data, and the site's home page, header and footer take the dashboard's look.",
    sections: [
      {
        kind: "features",
        items: [
          {
            text: "The dashboard menu marks the pages still to come as \"Soon\" instead of a lock, which read as a paid feature. Its new Preferences hold the language switch, and under 720 px the menu becomes a drawer.",
            technical: false,
          },
          {
            text: "A live indicator in the Overview header tells how fresh our indexing is (Live, Delayed, Stale or Offline). A click opens the last event, the Solana slot and the RPC latency, which leave the menu.",
            technical: false,
          },
          {
            text: "The site's home page shows an illustrated preview of the dashboard and how Yog-Scope works, in five steps.",
            technical: false,
          },
          {
            text: "The site's header and footer are rebuilt, with an \"Open the dashboard\" button, the version and a FR | EN switch that stays on the same page. The site takes the dashboard's colours and the Geist typeface, and its logos are vector images.",
            technical: false,
          },
        ],
      },
      {
        kind: "fixes",
        items: [
          {
            text: "The dashboard no longer waits for any data before it shows. The announcements and each Overview block arrive on their own, and a block that fails shows its own error instead of an error page.",
            technical: false,
          },
          {
            text: "The site and the API are served by a front web server shared with other sites on the same machine. Nothing changes for visitors.",
            technical: true,
          },
        ],
      },
    ],
  },
  {
    version: "v0.1.6",
    date: "2026-10-16",
    summary:
      "Pages no longer fail on a token logo that is not a link or on a mistyped pool address, and the network status is split between the Solana network and our own indexing.",
    sections: [
      {
        kind: "features",
        items: [
          {
            text: "The network status is split in two: /api/network gives the Solana network as our probe sees it (slot and latency), /api/ingestion gives how fresh our own indexing is. /api/network/status is gone. In the dashboard, the network panel loads each half on its own, so one that is slow or down no longer blanks the other.",
            technical: false,
          },
          {
            text: "The current state of a pool gives the dollar value of each of its two sides, under the same price-age limit as its TVL. Tokens embedded in a pool or a signal no longer carry a price, which had no age limit: the pool page's composition chart now uses the server's values, and is hidden when a side's price is missing or more than 15 minutes old; the signal card no longer shows a line of token prices.",
            technical: false,
          },
        ],
      },
      {
        kind: "fixes",
        items: [
          {
            text: "A token logo that is not an http or https link is now dropped, and the page shows the token's initial instead. A logo that was not a link at all used to make the whole page fail to load (pool list, pool page, signals) or the Overview's top-pools block show an error.",
            technical: false,
          },
          {
            text: "A pool page opened with a malformed or truncated address now shows \"Pool not found\". It used to fail with a server error, or show an error with a Retry button that could never work.",
            technical: false,
          },
          {
            text: "A security update of the TLS library used by the backend services (rustls 0.23.46).",
            technical: true,
          },
        ],
      },
    ],
  },
  {
    version: "v0.1.5",
    date: "2026-10-14",
    summary:
      "The Overview's figures load again: they now read the values computed once per hour instead of being recomputed at each request.",
    sections: [
      {
        kind: "features",
        items: [
          {
            text: "A new Yog-Scope logo and new browser icons, on the site and in the dashboard.",
            technical: false,
          },
        ],
      },
      {
        kind: "fixes",
        items: [
          {
            text: "The Overview's 24h volume, fees and total TVL now read the hourly values introduced in v0.1.4, instead of revaluing a full day of swaps at each request, which had made them fail to load. A figure the hourly service has not caught up with shows as — rather than as a partial sum.",
            technical: false,
          },
          {
            text: "Valuing the TVL of every pool now reads each token's latest price from a table that keeps one row per token, instead of searching the whole price history twice per pool. It was the slowest step of the hourly service.",
            technical: true,
          },
          {
            text: "The price-deviation signal reads its prices from the same table, instead of searching the whole price history twice per pool. It was the slowest step of the signal service.",
            technical: true,
          },
        ],
      },
    ],
  },
  {
    version: "v0.1.4",
    date: "2026-10-13",
    summary:
      "Groundwork for the Overview figures: swaps are now valued once per hour and kept, and the backup service's database access is narrowed.",
    sections: [
      {
        kind: "features",
        items: [
          {
            text: "A new background service values each pool's swaps in dollars once per hour, keeps the result, and freezes each hour ten minutes after it closes. Nothing reads these figures yet: the Overview's 24h volume, fees and TVL, which are still recomputed from every swap at each request and currently fail to load, will move to them once they cover a full day.",
            technical: true,
          },
        ],
      },
      {
        kind: "fixes",
        items: [
          {
            text: "The backup service now reads the application's data schema by schema, instead of through PostgreSQL's built-in read-all role, which reached beyond what a backup needs.",
            technical: true,
          },
        ],
      },
    ],
  },
  {
    version: "v0.1.3",
    date: "2026-10-09",
    summary:
      "The dashboard is now organised by protocol, and raw swaps are kept for a week so the database stays within its disk.",
    sections: [
      {
        kind: "features",
        items: [
          {
            text: "The dashboard menu is organised by protocol: Overview, then Meteora DAMM v2, Meteora DLMM, Raydium and Orca, then Token. Only Overview is open for now; the other entries carry a lock until their page exists. The pool list, signals and watchlist pages are no longer in the menu.",
            technical: false,
          },
          {
            text: "A new look for the dashboard frame: one colour per role, and the Geist typeface.",
            technical: false,
          },
        ],
      },
      {
        kind: "fixes",
        items: [
          {
            text: "Raw swap events are kept for 7 days instead of 30, and compressed one day after their day ends instead of seven: since every DAMM v2 pool is observed, they were outgrowing the disk. The hourly swap history that charts and analytics read is unaffected and still kept in full; a pool's list of individual swaps now goes back 7 days.",
            technical: false,
          },
          {
            text: "Next.js is updated to 16.3.8, following five advisories published on 7 October. None of them applied to this site's configuration.",
            technical: true,
          },
        ],
      },
    ],
  },
  {
    version: "v0.1.2",
    date: "2026-10-07",
    summary:
      "More regular price sampling: Jupiter requests are spaced instead of sent in bursts, and a restart keeps the prices a round had already received.",
    sections: [
      {
        kind: "fixes",
        items: [
          {
            text: "Requests to Jupiter are spaced under the configured rate limit (60 per minute by default) instead of sent in bursts, which Jupiter refused (HTTP 429) and which left tokens that have a price without one, round after round.",
            technical: false,
          },
          {
            text: "A token Jupiter answers without a price is asked again less and less often — up to every 15 minutes at the default 30-second cadence — instead of at every round; the next price it gets puts it back on every round. A request in which none of its 50 tokens gets a price is read as a failed answer, and changes no token's turn.",
            technical: true,
          },
          {
            text: "Stopping the enrichment service in the middle of a pricing round keeps the prices already received, instead of losing the whole round.",
            technical: true,
          },
        ],
      },
    ],
  },
  {
    version: "v0.1.1",
    date: "2026-10-06",
    summary: "A security update of the dashboard's image library.",
    sections: [
      {
        kind: "fixes",
        items: [
          {
            text: "sharp, the image library behind the dashboard, is updated to 0.35.5 for a vulnerability in the librsvg it bundles (CVE-2026-96889). Nothing else changes.",
            technical: true,
          },
        ],
      },
    ],
  },
  {
    version: "v0.1.0",
    date: "2026-10-06",
    summary:
      "The first production release: a real-time observer of Meteora DAMM v2 on Solana, with a signal engine that surfaces risk across every observed pool.",
    sections: [
      {
        kind: "features",
        items: [
          {
            text: "Real-time indexing of Meteora DAMM v2 events — swaps, liquidity, positions, fee updates and pool lifecycle — decoded from on-chain Anchor emissions, for an operator-selected set of pools; the other pools their transactions touch are discovered along the way.",
            technical: false,
          },
          {
            text: "Pool pages: composition, spot price decoded from the on-chain sqrt-price, realized-fees analytics with 30-day fee and effective-rate charts, and Swaps, Liquidity and Alerts tabs.",
            technical: false,
          },
          {
            text: "Pools list with a fee column and a fee-tier filter, and a watchlist of starred pools, kept in the browser.",
            technical: false,
          },
          {
            text: "Overview with global KPIs — total TVL, 24h volume and fees, pools discovered — a top-pools ranking and the latest signals.",
            technical: false,
          },
          {
            text: "Signal engine with three risk detectors: flow imbalance, price–oracle deviation, and TVL drain (rug-like liquidity exodus).",
            technical: false,
          },
          {
            text: "Live signal feed on /signals — streamed over SSE, with severity × detector filters and per-detector explanations — and a worst-severity indicator on the pools list, with a hover detail.",
            technical: false,
          },
          {
            text: "Token enrichment: metadata, USD prices and pool-account backfill, independent from the ingestion path. Jupiter rate-limits (429) are retried with backoff, and every provider call carries a timeout.",
            technical: false,
          },
          {
            text: "Operator announcements: a dismissible banner for maintenance, incidents, releases and beta notes, published without a deploy.",
            technical: false,
          },
          {
            text: "Dashboard: collapsible sidebar with a network-status panel (Solana slot, RPC latency), metric definitions behind info popovers, an English and French interface, and a privacy notice that lists every processing and transfer.",
            technical: false,
          },
          {
            text: "Hardened serving: the API bounds its memory, connections and heavy requests — past its limits it turns heavy requests and streams away instead of running out of memory — and the dashboard sends HSTS and a content security policy that blocks framing and limits where it connects.",
            technical: true,
          },
        ],
      },
    ],
  },
] as const;
