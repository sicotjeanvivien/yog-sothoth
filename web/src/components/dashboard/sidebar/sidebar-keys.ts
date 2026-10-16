/**
 * The sidebar's keys, as literal unions. Moving a key from `ClosedNavKey` to `OpenNavKey`
 * makes the icon map refuse to compile until the entry has an icon.
 */

export type OpenNavKey = "overview";

export type ClosedNavKey =
  | "meteoraDammV2"
  | "meteoraDlmm"
  | "raydium"
  | "orca"
  | "token";
