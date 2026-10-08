/**
 * Sidebar navigation keys.
 *
 * Literal unions, so TypeScript rejects a typo at compile time. Leaf
 * module — it imports nothing.
 *
 * An entry is open (it has a page) or closed (visible, leads nowhere).
 * Opening one moves its key from `ClosedNavKey` to `OpenNavKey`; the
 * sidebar's icon map then refuses to compile until the entry has an
 * icon for the collapsed rail.
 */

export type OpenNavKey = "overview";

export type ClosedNavKey =
  | "meteoraDammV2"
  | "meteoraDlmm"
  | "raydium"
  | "orca"
  | "token";
