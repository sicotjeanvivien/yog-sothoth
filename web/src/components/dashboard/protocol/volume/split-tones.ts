/** The tints of a split bar, read by the server's legend and the client's bars alike. */

export type SplitTone = "in" | "out" | "unsplit";

export const SPLIT_TONE: Record<SplitTone, string> = {
  in: "bg-dash-run",
  out: "bg-dash-run-out",
  unsplit: "bg-dash-ink-3/45",
};
