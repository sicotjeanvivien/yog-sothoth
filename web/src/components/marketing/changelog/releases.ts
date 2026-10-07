/**
 * Changelog content — one entry per release.
 *
 * Pure data, no React (the `marketing-footer-links` pattern). This is
 * the file the operator edits at each release; the vitest suite next
 * to it guards the invariants (unique versions, `vX.Y.Z` format,
 * newest first).
 *
 * Entries are written in English only — the v1 language decision shared
 * with operator announcements: the page chrome (title, section labels)
 * is i18n'd, the free release copy is not.
 *
 * `version` doubles as the block's anchor id: a release announcement
 * published in the `announcements` table points at
 * `/changelog#<version>` via its `link_url`.
 */

export type ReleaseSectionKind = "features" | "fixes";

export type ReleaseSection = {
  /** Which section label the chrome shows (i18n key). */
  kind: ReleaseSectionKind;
  /** Free English copy, one bullet per item. */
  items: readonly string[];
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
    version: "v0.1.2",
    date: "2026-10-07",
    summary:
      "More regular price sampling: Jupiter requests are spaced instead of sent in bursts, and a restart keeps the prices a round had already received.",
    sections: [
      {
        kind: "fixes",
        items: [
          "Requests to Jupiter are spaced under the configured rate limit (60 per minute by default) instead of sent in bursts, which Jupiter refused (HTTP 429) and which left tokens that have a price without one, round after round.",
          "A token Jupiter answers without a price is asked again less and less often — up to every 15 minutes at the default 30-second cadence — instead of at every round; the next price it gets puts it back on every round.",
          "Stopping the enrichment service in the middle of a pricing round keeps the prices already received, instead of losing the whole round.",
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
          "sharp, the image library behind the dashboard, is updated to 0.35.5 for a vulnerability in the librsvg it bundles (CVE-2026-96889). Nothing else changes.",
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
          "Real-time indexing of Meteora DAMM v2 events — swaps, liquidity, positions, fee updates and pool lifecycle — decoded from on-chain Anchor emissions, for an operator-selected set of pools; the other pools their transactions touch are discovered along the way.",
          "Pool pages: composition, spot price decoded from the on-chain sqrt-price, realized-fees analytics with 30-day fee and effective-rate charts, and Swaps, Liquidity and Alerts tabs.",
          "Pools list with a fee column and a fee-tier filter, and a watchlist of starred pools, kept in the browser.",
          "Overview with global KPIs — total TVL, 24h volume and fees, pools discovered — a top-pools ranking and the latest signals.",
          "Signal engine with three risk detectors: flow imbalance, price–oracle deviation, and TVL drain (rug-like liquidity exodus).",
          "Live signal feed on /signals — streamed over SSE, with severity × detector filters and per-detector explanations — and a worst-severity indicator on the pools list, with a hover detail.",
          "Token enrichment: metadata, USD prices and pool-account backfill, independent from the ingestion path. Jupiter rate-limits (429) are retried with backoff, and every provider call carries a timeout.",
          "Operator announcements: a dismissible banner for maintenance, incidents, releases and beta notes, published without a deploy.",
          "Dashboard: collapsible sidebar with a network-status panel (Solana slot, RPC latency), metric definitions behind info popovers, an English and French interface, and a privacy notice that lists every processing and transfer.",
          "Hardened serving: the API bounds its memory, connections and heavy requests — past its limits it turns heavy requests and streams away instead of running out of memory — and the dashboard sends HSTS and a content security policy that blocks framing and limits where it connects.",
        ],
      },
    ],
  },
] as const;
