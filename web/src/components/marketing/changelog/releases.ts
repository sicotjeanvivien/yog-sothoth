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
    version: "v0.1.1",
    date: "2026-07-20",
    summary:
      "Yog-Scope grows from an observer into an alerting system: a signal engine watches every observed pool and surfaces risk across the dashboard.",
    sections: [
      {
        kind: "features",
        items: [
          "Signal engine with three risk detectors: flow imbalance, price–oracle deviation, and TVL drain (rug-like liquidity exodus).",
          "Live signal feed on /signals — streamed over SSE, with severity × detector filters and per-detector explanations.",
          "Alerts tab on every pool page, and a worst-severity signal indicator on the pools list with a hover detail popover.",
          "Operator announcements: a dismissible banner on the dashboard for maintenance, incidents, releases and beta notes — published without a deploy.",
          "Dashboard UX pass: collapsible sidebar, metric definitions behind info popovers, slimmer page headers, and a global text-scale bump for readability.",
        ],
      },
      {
        kind: "fixes",
        items: [
          "Jupiter price rate-limits (429) are retried with pacing instead of dropping the price chunk.",
          "Provider HTTP calls carry timeouts — a hung provider can no longer silently freeze token enrichment.",
          "Single shared Docker builder stage for the five backend images — faster, leaner builds.",
        ],
      },
    ],
  },
  {
    version: "v0.1.0",
    date: "2026-06-30",
    summary:
      "The foundation: a protocol-centric, real-time observer of Meteora DAMM v2 activity on Solana — pools are discovered from the transaction stream, not configured.",
    sections: [
      {
        kind: "features",
        items: [
          "Real-time indexing of Meteora DAMM v2 events: swaps, liquidity, positions, fee updates and pool lifecycle, decoded from on-chain Anchor emissions.",
          "Pool pages: composition, spot price decoded from the on-chain sqrt-price, realized-fees analytics and 30-day activity charts.",
          "Overview with global KPIs — total TVL, 24h volume and fees, pools discovered — and a top-pools ranking.",
          "Token enrichment daemon: metadata, USD prices and pool-account backfill, independent from the ingestion path.",
          "English and French interface.",
        ],
      },
    ],
  },
] as const;
