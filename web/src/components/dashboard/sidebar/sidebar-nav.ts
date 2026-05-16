/**
 * Sidebar navigation configuration.
 *
 * Pure data — no React, no JSX. Describes *which* entries the sidebar
 * renders, in *what order*, pointing at *which routes*. The component
 * (`sidebar.tsx`) consumes this list and is responsible only for
 * turning it into markup.
 *
 * Separating the data from the presentation keeps the component a
 * dumb renderer and makes the nav structure trivial to inspect or
 * test in isolation.
 */

import type { SidebarNavKey } from "./sidebar-keys";

/**
 * A single navigation entry.
 *
 * - `key`      stable identity, also used to resolve the active item.
 * - `href`     route path *without* the locale segment. next-intl's
 *              `Link` prepends the active locale at render time, so
 *              `/overview` becomes `/fr/overview` transparently.
 * - `labelKey` i18n key, relative to the `Sidebar.nav` namespace.
 *              The component resolves it via `useTranslations`.
 *              The config itself stays language-agnostic.
 */
export type SidebarNavItem = {
  key: SidebarNavKey;
  href: string;
  labelKey: string;
};

/**
 * The navigation entries, in display order.
 *
 * `readonly` + `as const` so neither the array nor its entries can be
 * mutated at runtime — the config is a constant, not a mutable store.
 */
export const SIDEBAR_NAV: readonly SidebarNavItem[] = [
  { key: "overview", href: "/overview", labelKey: "overview" },
  { key: "pools", href: "/pools", labelKey: "pools" },
] as const;