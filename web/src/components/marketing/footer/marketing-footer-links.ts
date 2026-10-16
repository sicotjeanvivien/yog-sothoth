/**
 * The footer's links, as data. Apart from the nav's on purpose: the two surfaces list
 * different things.
 */

/** `kind`: a locale `route` (next-intl Link), an in-page `anchor`, or an `external` URL in a new tab. */
export type FooterLink = {
  key: string;
  labelKey: string;
  href: string;
  kind: "route" | "anchor" | "external";
};

/** `headingKey` and each `labelKey` resolve against `namespace`. */
export type FooterColumn = {
  key: string;
  namespace: string;
  links: readonly FooterLink[];
};

const PRODUCT_COLUMN: FooterColumn = {
  key: "product",
  namespace: "Marketing.footer.product",
  links: [
    { key: "overview", labelKey: "overview", href: "/overview", kind: "route" },
    {
      key: "features",
      labelKey: "features",
      href: "/#features",
      kind: "anchor",
    },
    {
      key: "changelog",
      labelKey: "changelog",
      href: "/changelog",
      kind: "route",
    },
    {
      key: "support-us",
      labelKey: "support-us",
      href: "/support-us",
      kind: "route",
    },
  ],
};

const COMPANY_COLUMN: FooterColumn = {
  key: "company",
  namespace: "Marketing.footer.company",
  links: [
    { key: "about", labelKey: "about", href: "/about", kind: "route" },
    { key: "privacy", labelKey: "privacy", href: "/privacy", kind: "route" },
    {
      key: "legal-notice",
      labelKey: "legal-notice",
      href: "/legal-notice",
      kind: "route",
    },
    { key: "terms", labelKey: "terms", href: "/terms", kind: "route" },
  ],
};

export const FOOTER_COLUMNS: readonly FooterColumn[] = [
  PRODUCT_COLUMN,
  COMPANY_COLUMN,
] as const;
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

export const FOOTER_AWSD = {
  href: "https://awsd.fr/",
} as const;
