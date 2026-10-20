/**
 * The Overview's grid, ruled like the canvas: cells on `surface`, 1 px rules between them;
 * `adv:` tightens it. ⚠️ Each block and its waiting slot share span and height in both modes.
 */

export const GRID_CLASS = "grid grid-cols-12 border-l border-dash-rule";

export const CELL_CLASS =
  "min-w-0 border-r border-b border-dash-rule bg-dash-surface p-4 rail:px-6 rail:pt-5 rail:pb-[22px] adv:rail:px-4 adv:rail:pt-3.5 adv:rail:pb-4";

export const SPAN = {
  kpi: "col-span-6 dash-wide:col-span-3 adv:rail:col-span-4 adv:dash-wide:col-span-2",
  main: "col-span-12 dash-wide:col-span-8",
  side: "col-span-12 dash-wide:col-span-4 dash-wide:row-span-3",
} as const;

export const MIN_HEIGHT = {
  kpi: "min-h-[148px] rail:min-h-[136px]",
  protocols: "min-h-[262px] adv:min-h-[247px]",
  signals: "min-h-[420px]",
  volume: "min-h-[318px] adv:min-h-[434px]",
  tokens: "min-h-[530px] adv:min-h-[430px]",
} as const;

/** A block's heading row: its title and ⓘ, then what sits at its right. */
export const BLOCK_HEAD_CLASS =
  "mb-3.5 adv:mb-2.5 flex flex-wrap items-baseline justify-between gap-x-3 gap-y-2";

export const BLOCK_TITLE_CLASS = "text-[16px] font-semibold text-dash-ink";

export const LABEL_CLASS =
  "font-dash-mono text-[11px] font-medium tracking-[0.12em] text-dash-ink-3 uppercase";

/** A table row; its columns come with each table. */
export const ROW_CLASS =
  "grid items-baseline gap-x-3 border-b border-dash-rule py-[9px] adv:py-1.5 text-[14px] last:border-b-0";

export const HEAD_ROW_CLASS =
  "grid items-baseline gap-x-3 border-b border-dash-rule pb-2 font-dash-mono text-[11px] font-medium tracking-[0.1em] whitespace-nowrap text-dash-ink-3 uppercase";

export const NUM_CLASS = "text-right font-dash-mono tabular-nums whitespace-nowrap";

/** Shown in the advanced mode only, as a flex box or a block; the server renders it in both. */
export const ADVANCED_ONLY = { flex: "hidden adv:flex", block: "hidden adv:block" } as const;

/** A KPI cell, and its waiting slot: the same box in both modes. */
export function kpiCellClass(advanced: boolean): string {
  return `${CELL_CLASS} ${SPAN.kpi} ${MIN_HEIGHT.kpi} ${advanced ? ADVANCED_ONLY.flex : "flex"} flex-col gap-1.5 adv:gap-1`;
}
