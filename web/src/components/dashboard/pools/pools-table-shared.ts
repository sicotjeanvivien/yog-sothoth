/**
 * The pool table's grid, cell classes and label types. ⚠️ No React, no server import: the
 * server `PoolsTable`, `PoolsTableRow` and the client watchlist all import it.
 */

// ⚠️ Volume is 144px, not 112: the coverage mark does not fit in 112, and the spill goes left
// (`justify-end`), unseen by `scrollWidth`. Any change moves `TABLE_MIN_WIDTH_CLASS` with it.
export const GRID_COLS =
  "grid-cols-[minmax(190px,1.8fr)_minmax(84px,0.5fr)_minmax(112px,0.9fr)_minmax(84px,0.6fr)_minmax(112px,0.9fr)_minmax(144px,0.9fr)_minmax(112px,0.9fr)_minmax(112px,0.9fr)_minmax(104px,0.7fr)]";

/** Below it the table scrolls rather than squashes. ⚠️ It moves with any track. */
export const TABLE_MIN_WIDTH_CLASS = "min-w-[1062px]";

const HEAD_CELL_BASE =
  "flex items-center px-4 py-3 text-[11px] font-medium tracking-[0.06em] text-slate-500 uppercase whitespace-nowrap";
export const HEAD_CELL_CLASS = HEAD_CELL_BASE;
export const HEAD_CELL_NUMERIC_CLASS = `${HEAD_CELL_BASE} justify-end`;
export const HEAD_CELL_SORTABLE_CLASS = "flex items-center px-4 py-3";

export const CELL_CLASS =
  "px-4 py-3 text-[14px] text-slate-300 align-middle whitespace-nowrap flex items-center";
export const CELL_NUMERIC_CLASS = `${CELL_CLASS} justify-end font-mono`;

/** The coverage sentence of a row's 24h volume, passed in like `SignalCellLabels`. */
export type CoverageLabel = (priced: number, total: number) => string;

/** ⚠️ Resolved by the table and passed as strings: the row renders in both a server tree
 *  (`/pools`) and a client one (`/watchlist`), so it never translates itself. */
export type SignalCellLabels = {
  /** Localized detector tag; falls back to the raw detector name. */
  tagFor: (detector: string) => string;
  /** Accessible name of the indicator, given the signal count. */
  ariaFor: (count: number) => string;
  /** Popover heading. */
  title: string;
};
