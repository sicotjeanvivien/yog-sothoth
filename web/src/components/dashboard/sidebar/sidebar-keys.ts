/**
 * The sidebar's keys, as literal unions. Every key, open or closed, needs its icon in
 * `NAV_ICONS`: the collapsed rail shows nothing else.
 */

export type OpenNavKey = "overview";

export type ClosedNavKey =
  | "meteoraDammV2"
  | "meteoraDlmm"
  | "raydium"
  | "orca"
  | "token";

export type NavKey = OpenNavKey | ClosedNavKey;
