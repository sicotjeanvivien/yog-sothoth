/**
 * The sidebar's keys, as literal unions. Every row but a platform's products needs its icon in
 * `NAV_ICONS`: the collapsed rail shows nothing else.
 */

export type OpenNavKey = "overview";

export type ClosedNavKey = "token";

export type NavKey = OpenNavKey | ClosedNavKey;

export type PlatformKey = "meteora" | "raydium" | "orca";

/** The entries listed under a platform, without an icon of their own. */
export type ProductKey = "meteoraDammV2" | "meteoraDlmm";

export type IconKey = NavKey | PlatformKey;
