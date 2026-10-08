/**
 * Sidebar navigation configuration.
 *
 * Pure data — no React, no JSX. Describes *which* entries the sidebar
 * renders, in *what order*, pointing at *which routes*. The renderer
 * (`sidebar-nav-list.tsx`) only turns it into markup.
 */

import type { ClosedNavKey, OpenNavKey } from "./sidebar-keys";

/**
 * An entry with a page.
 *
 * - `href`     route path *without* the locale segment; next-intl's
 *              `Link` prepends the active locale at render time.
 * - `labelKey` i18n key, relative to `Dashboard.Sidebar.nav`.
 */
export type OpenNavEntry = {
  kind: "open";
  key: OpenNavKey;
  href: string;
  labelKey: string;
};

/**
 * An entry without a page: shown with a lock, never a link. It has no
 * `href` on purpose — there is nothing to point at.
 */
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

/** The navigation, in display order. */
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
