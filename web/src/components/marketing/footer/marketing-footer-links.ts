/**
 * Marketing footer links.
 *
 * Pure data — no React, no JSX. The footer's link column is short by
 * design: the product is early, so it lists only destinations that
 * actually exist (Overview, Features, Support). It will grow as real
 * pages appear (Docs, Pricing, …).
 *
 * Kept separate from `marketing-nav-links.ts` on purpose — a footer
 * and a nav don't list the same things, and forcing a shared list
 * would couple two surfaces that evolve independently. The two
 * configs may overlap; that's fine.
 */

/**
 * A single footer link.
 *
 * - `key`      stable identity, also the React list key.
 * - `labelKey` i18n key, relative to the `Marketing.footer.links`
 *              namespace.
 * - `href`     internal locale-free path, in-page anchor, or full
 *              external URL — see `kind`.
 * - `kind`     "route"   internal locale route (next-intl Link);
 *              "anchor"  in-page fragment (plain anchor, no locale);
 *              "external" full URL (plain anchor, new tab).
 */
export type FooterLink = {
  key: string;
  labelKey: string;
  href: string;
  kind: "route" | "anchor" | "external";
};

/** The footer link column, in display order. */
export const FOOTER_LINKS: readonly FooterLink[] = [
  { key: "overview", labelKey: "overview", href: "/overview", kind: "route" },
  { key: "features", labelKey: "features", href: "/#features", kind: "anchor" },
  { key: "support", labelKey: "support", href: "/support", kind: "route" },
] as const;

/**
 * Social links shown under the brand block. Both external.
 * `icon` keys map to glyphs in the footer component.
 */
export type FooterSocial = {
  key: string;
  label: string;
  href: string;
  icon: "x" | "github";
};

export const FOOTER_SOCIALS: readonly FooterSocial[] = [
  {
    key: "x",
    label: "X",
    href: "https://x.com/AWSD_JV",
    icon: "x",
  },
  {
    key: "github",
    label: "GitHub",
    href: "https://github.com/sicotjeanvivien/yog-sothoth",
    icon: "github",
  },
] as const;

/** The "Built by AWSD" credit box — external link to the AWSD site. */
export const FOOTER_AWSD = {
  href: "https://awsd.fr/",
} as const;