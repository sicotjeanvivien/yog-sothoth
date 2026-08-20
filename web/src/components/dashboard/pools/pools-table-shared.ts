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

// ⚠️ The volume track is 144px where its neighbours are 112: it is the only
// one that can carry the coverage mark (`VolumeCoverageMark`), and the mark
// does not fit in 112. Measured in the browser at the pinned width, on the
// widest content the cell can hold — an 8-character `$833.33K` next to a
// two-digit `20/23`:
//
//     amount 67.4px + gap 6px + mark 33.1px  =  106.6px of content
//     112px track → 80px content box         →  26.6px SPILLS
//     144px track → 112px content box        →  5.4px to spare
//
// The spill goes LEFT, because the cell is `justify-end` — which is why
// `scrollWidth` does not see it (it only counts overflow to the right) and why
// a screenshot shows nothing until the neighbouring cell's own padding is
// eaten through. Measure with a Range over the amount's text node, not with
// scrollWidth, if you touch this again.
//
// Cost, stated so it can be argued with: the table now scrolls 32px earlier
// than it did. Any change to either number must keep TABLE_MIN_WIDTH_CLASS in
// step — see below.
export const GRID_COLS =
  "grid-cols-[minmax(190px,1.8fr)_minmax(84px,0.5fr)_minmax(112px,0.9fr)_minmax(84px,0.6fr)_minmax(112px,0.9fr)_minmax(144px,0.9fr)_minmax(112px,0.9fr)_minmax(112px,0.9fr)_minmax(104px,0.7fr)]";

/** Min width below which the table scrolls horizontally instead of squashing.
 *  Sized to the sum of the column minimums (~1054px) so the grid uses those
 *  minimums exactly, with no forced extra slack. Trimmed once the protocol
 *  cell (icon + "DAMM v2") and the relative-time cells ("il y a 2 h") went
 *  compact — the old 1232px predated that and forced an avoidable scroll;
 *  +32px when the volume track went to 144 for the coverage mark. Measured,
 *  not assumed: at this width the grid pins every track to its minimum
 *  exactly (volume cell = 144px on the nose), so this constant IS the sum and
 *  has to move with it. */
export const TABLE_MIN_WIDTH_CLASS = "min-w-[1062px]";

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

/** Resolves the coverage sentence of one row's 24h volume. Passed in for the
 *  same reason as `SignalCellLabels`: the row renders in a server tree
 *  (`/pools`) and a client one (`/watchlist`), so it never translates itself. */
export type CoverageLabel = (priced: number, total: number) => string;

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
