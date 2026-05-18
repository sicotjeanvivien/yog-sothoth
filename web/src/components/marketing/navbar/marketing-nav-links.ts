/**
 * Marketing navigation links.
 *
 * Pure data — no React, no JSX. Describes which links the marketing
 * nav renders, in what order, and crucially whether each one is an
 * internal (locale-prefixed) route or an external URL.
 *
 * The component (`marketing-nav.tsx`) reads this list and is
 * responsible only for turning it into markup — choosing the
 * next-intl `Link` for internal entries and a plain anchor for
 * external ones.
 */

/**
 * A single nav link.
 *
 * - `key`      stable identity, also used as the React list key.
 * - `labelKey` i18n key, relative to the `Marketing.nav` namespace.
 * - `href`     for internal links, a locale-free path (`/support`);
 *              for external links, a full URL (`https://…`).
 * - `external` discriminates the two. Internal links go through
 *              next-intl's `Link` (locale prefix added automatically);
 *              external links render as a plain anchor opened in a
 *              new tab.
 */
export type MarketingNavLink = {
  key: string;
  labelKey: string;
  href: string;
  external: boolean;
};

/**
 * The nav links, in display order.
 *
 * Kept deliberately short — Overview points at the dashboard,
 * Support at the marketing support page, GitHub at the public repo.
 * Pricing / Docs are intentionally absent until they exist.
 */
export const MARKETING_NAV_LINKS: readonly MarketingNavLink[] = [
  { key: "overview", labelKey: "overview", href: "/overview", external: false },
  { key: "support", labelKey: "support", href: "/support", external: false },
  {
    key: "github",
    labelKey: "github",
    href: "https://github.com/sicotjeanvivien/yog-sothoth",
    external: true,
  },
] as const;