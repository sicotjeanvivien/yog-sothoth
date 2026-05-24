/**
 * Marketing footer links.
 *
 * Pure data — no React, no JSX. The footer has two link columns:
 *
 *   - Product : destinations that already exist (Overview, Features,
 *               Support);
 *   - Company : About / Privacy / Terms — these pages do NOT exist
 *               yet and currently 404. They are listed deliberately;
 *               the routes are stubbed below with TODOs so the pages
 *               can be wired in later without hunting for the spot.
 *
 * Kept separate from `marketing-nav-links.ts` on purpose — a footer
 * and a nav don't list the same things, and forcing a shared list
 * would couple two surfaces that evolve independently.
 */
 
/**
 * A single footer link.
 *
 * - `key`      stable identity, also the React list key.
 * - `labelKey` i18n key, relative to its column's namespace.
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

/**
 * A footer link column — a heading plus its links. `headingKey` and
 * each link's `labelKey` resolve against `namespace`.
 */
export type FooterColumn = {
  key: string;
  /** i18n namespace holding `heading` and the link labels. */
  namespace: string;
  links: readonly FooterLink[];
};
 
/** Product column — all destinations exist today. */
const PRODUCT_COLUMN: FooterColumn = {
  key: "product",
  namespace: "Marketing.footer.product",
  links: [
    { key: "overview", labelKey: "overview", href: "/overview", kind: "route" },
    { key: "features", labelKey: "features", href: "/#features", kind: "anchor" },
    { key: "support-us", labelKey: "support-us", href: "/support-us", kind: "route" },
  ],
};
 
/**
 * Company column.
 *
 * TODO: these three routes do not exist yet — they 404 until the
 * pages are created under `(marketing)/`:
 *   - /about    company / project background (editorial);
 *   - /privacy  privacy policy — REQUIRED under GDPR once the site
 *               is publicly deployed;
 *   - /terms    terms of use.
 * France also needs separate "mentions légales". Treat this as a
 * v0.1-deployment task, not a footer detail.
 */
const COMPANY_COLUMN: FooterColumn = {
  key: "company",
  namespace: "Marketing.footer.company",
  links: [
    { key: "about", labelKey: "about", href: "/about", kind: "route" },
    { key: "privacy", labelKey: "privacy", href: "/privacy", kind: "route" },
    { key: "terms", labelKey: "terms", href: "/terms", kind: "route" },
  ],
};
 
/** The footer link columns, in display order. */
export const FOOTER_COLUMNS: readonly FooterColumn[] = [
  PRODUCT_COLUMN,
  COMPANY_COLUMN,
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