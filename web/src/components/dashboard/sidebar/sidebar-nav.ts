/** The sidebar's navigation, as data, in display order. */

import type { ClosedNavKey, OpenNavKey, PlatformKey, ProductKey } from "./sidebar-keys";

/** `href` without the locale, which next-intl's `Link` prepends. */
export type OpenNavEntry<K extends string = OpenNavKey> = {
  kind: "open";
  key: K;
  href: string;
  labelKey: string;
};

/** An entry without a page yet, tagged "Soon", never a link. */
export type ClosedNavEntry<K extends string = ClosedNavKey> = {
  kind: "closed";
  key: K;
  labelKey: string;
};

export type SidebarNavEntry = OpenNavEntry | ClosedNavEntry;

/** A product opens by turning `open`, with its `href`; its key does not change. */
export type ProductEntry = OpenNavEntry<ProductKey> | ClosedNavEntry<ProductKey>;

/**
 * A link to its first open product; with none, a caption over its products, or a closed entry
 * tagged "Soon" when it lists none.
 */
export type SidebarNavPlatform = {
  kind: "platform";
  key: PlatformKey;
  labelKey: string;
  products: readonly ProductEntry[];
};

export type SidebarNavItem = SidebarNavEntry | SidebarNavPlatform;

export const PLATFORMS: readonly SidebarNavPlatform[] = [
  {
    kind: "platform",
    key: "meteora",
    labelKey: "meteora",
    products: [
      { kind: "open", key: "meteoraDammV2", href: "/protocols/meteora-damm-v2", labelKey: "dammV2" },
      { kind: "closed", key: "meteoraDlmm", labelKey: "dlmm" },
    ],
  },
  { kind: "platform", key: "raydium", labelKey: "raydium", products: [] },
  { kind: "platform", key: "orca", labelKey: "orca", products: [] },
];

export const SIDEBAR_NAV: readonly SidebarNavItem[] = [
  { kind: "open", key: "overview", href: "/overview", labelKey: "overview" },
  ...PLATFORMS,
  { kind: "closed", key: "token", labelKey: "token" },
];

/** The About page's scope: one line per product, or per platform that lists none. */
export type ScopeLine = { key: ProductKey | PlatformKey; labelKeys: string[] };

export const PROTOCOL_SCOPE: readonly ScopeLine[] = PLATFORMS.flatMap(
  (platform): ScopeLine[] =>
    platform.products.length > 0
      ? platform.products.map((product) => ({
          key: product.key,
          labelKeys: [platform.labelKey, product.labelKey],
        }))
      : [{ key: platform.key, labelKeys: [platform.labelKey] }],
);
