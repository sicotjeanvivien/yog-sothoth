/** The marketing nav's links, as data, in display order. */

/** `href` is a locale-free path, or a full URL when `external` (a plain anchor in a new tab). */
export type MarketingNavLink = {
  key: string;
  labelKey: string;
  href: string;
  external: boolean;
};

export const MARKETING_NAV_LINKS: readonly MarketingNavLink[] = [
  { key: "features", labelKey: "features", href: "/#features", external: false },
  { key: "support-us", labelKey: "support-us", href: "/support-us", external: false },
  {
    key: "github",
    labelKey: "github",
    href: "https://github.com/sicotjeanvivien/yog-sothoth",
    external: true,
  },
] as const;
