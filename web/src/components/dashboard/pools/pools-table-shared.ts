/**
 * Shared vocabulary for the pool table — grid template, cell/header classes
 * and the signal-label type.
 *
 * A neutral module (no React, no server imports) so every consumer can share
 * it without dragging another's runtime in: the server `PoolsTable`, the
 * shared `PoolsTableRow`, and the client-side watchlist all import from here.
 *
 * Column order (nine): pair · signals · protocol · fee · TVL · volume 24h ·
 * first seen · last seen · actions. The actions column trails every row with
 * the per-pool utilities (copy address, Solscan, watchlist star).
 */

export const GRID_COLS =
  "grid-cols-[minmax(200px,1.8fr)_minmax(90px,0.5fr)_minmax(140px,1fr)_minmax(90px,0.6fr)_minmax(120px,0.9fr)_minmax(120px,0.9fr)_minmax(130px,1fr)_minmax(130px,1fr)_minmax(112px,0.7fr)]";

/** Min width below which the table scrolls horizontally instead of squashing. */
export const TABLE_MIN_WIDTH_CLASS = "min-w-[1232px]";

// ── Header cells ──────────────────────────────────────────────────────
// Deliberately understated (11px, medium weight, dim, tight tracking) so the
// column titles frame the data without competing with it.
const HEAD_CELL_BASE =
  "flex items-center px-4 py-3 text-[11px] font-medium tracking-[0.06em] text-slate-500 uppercase whitespace-nowrap";
export const HEAD_CELL_CLASS = HEAD_CELL_BASE;
export const HEAD_CELL_NUMERIC_CLASS = `${HEAD_CELL_BASE} justify-end`;
export const HEAD_CELL_SORTABLE_CLASS = "flex items-center px-4 py-3";

// ── Body cells ────────────────────────────────────────────────────────
export const CELL_CLASS =
  "px-4 py-3 text-[14px] text-slate-300 align-middle whitespace-nowrap flex items-center";
export const CELL_NUMERIC_CLASS = `${CELL_CLASS} justify-end font-mono`;

/** Labels the client-side signal cell needs, resolved once by the table and
 *  passed down as plain strings so the row stays i18n-agnostic. */
export type SignalCellLabels = {
  /** Localized detector tag; falls back to the raw detector name. */
  tagFor: (detector: string) => string;
  /** Accessible name of the indicator, given the signal count. */
  ariaFor: (count: number) => string;
  /** Popover heading. */
  title: string;
};
