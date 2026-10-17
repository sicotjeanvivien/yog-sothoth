/** The sidebar's navigation, as data, in display order. */

import type { ClosedNavKey, OpenNavKey } from "./sidebar-keys";

/** `href` without the locale, which next-intl's `Link` prepends. */
export type OpenNavEntry = {
  kind: "open";
  key: OpenNavKey;
  href: string;
  labelKey: string;
};

/** An entry without a page yet, tagged "Soon", never a link. */
export type ClosedNavEntry = {
  kind: "closed";
  key: ClosedNavKey;
  labelKey: string;
};

export type SidebarNavEntry = OpenNavEntry | ClosedNavEntry;

/** A caption over indented entries — not itself navigable. */
export type SidebarNavGroup = {
  kind: "group";
  labelKey: string;
  entries: readonly SidebarNavEntry[];
};

export type SidebarNavItem = SidebarNavEntry | SidebarNavGroup;

export const SIDEBAR_NAV: readonly SidebarNavItem[] = [
  { kind: "open", key: "overview", href: "/overview", labelKey: "overview" },
  {
    kind: "group",
    labelKey: "protocol",
    entries: [
      {
        kind: "closed",
        key: "meteoraDammV2",
        labelKey: "meteoraDammV2",
      },
      {
        kind: "closed",
        key: "meteoraDlmm",
        labelKey: "meteoraDlmm",
      },
      {
        kind: "closed",
        key: "raydium",
        labelKey: "raydium",
      },
      {
        kind: "closed",
        key: "orca",
        labelKey: "orca",
      },
    ],
  },
  { kind: "closed", key: "token", labelKey: "token" },
];
