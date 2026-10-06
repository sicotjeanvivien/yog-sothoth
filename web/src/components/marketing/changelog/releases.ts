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
    version: "v0.1.0",
    date: "2026-10-06",
    summary:
      "The first production release: a real-time observer of Meteora DAMM v2 on Solana, with a signal engine that surfaces risk across every observed pool.",
    sections: [
      {
        kind: "features",
        items: [
          "Real-time indexing of Meteora DAMM v2 events — swaps, liquidity, positions, fee updates and pool lifecycle — decoded from on-chain Anchor emissions. Pools are discovered from the transaction stream, not configured.",
          "Pool pages: composition, spot price decoded from the on-chain sqrt-price, realized-fees analytics, 30-day activity charts, and an Alerts tab.",
          "Overview with global KPIs — total TVL, 24h volume and fees, pools discovered — and a top-pools ranking.",
          "Signal engine with three risk detectors: flow imbalance, price–oracle deviation, and TVL drain (rug-like liquidity exodus).",
          "Live signal feed on /signals — streamed over SSE, with severity × detector filters and per-detector explanations — and a worst-severity indicator on the pools list.",
          "Token enrichment: metadata, USD prices and pool-account backfill, independent from the ingestion path.",
          "Operator announcements: a dismissible banner for maintenance, incidents, releases and beta notes.",
          "English and French interface, with a privacy notice that lists every processing and transfer.",
        ],
      },
      {
        kind: "fixes",
        items: [
          "Jupiter price rate-limits (429) are retried with pacing instead of dropping the price chunk.",
          "Provider HTTP calls carry timeouts — a hung provider can no longer silently freeze token enrichment.",
          "The API bounds its connections and its heavy requests, so a single client can no longer take it down.",
          "Security headers on the dashboard: a content security policy, and HSTS.",
        ],
      },
    ],
  },
] as const;
