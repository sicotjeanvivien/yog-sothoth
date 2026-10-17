/**
 * The footer's links, as data. Apart from the nav's on purpose: the two surfaces list
 * different things.
 */

/** `href` is a locale-free path, an in-page anchor of the home page included. */
export type FooterLink = {
  key: string;
  labelKey: string;
  href: string;
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
    { key: "dashboard", labelKey: "dashboard", href: "/overview" },
    { key: "features", labelKey: "features", href: "/#features" },
    { key: "changelog", labelKey: "changelog", href: "/changelog" },
    { key: "support-us", labelKey: "support-us", href: "/support-us" },
  ],
};

const COMPANY_COLUMN: FooterColumn = {
  key: "company",
  namespace: "Marketing.footer.company",
  links: [
    { key: "about", labelKey: "about", href: "/about" },
    { key: "privacy", labelKey: "privacy", href: "/privacy" },
    { key: "legal-notice", labelKey: "legal-notice", href: "/legal-notice" },
    { key: "terms", labelKey: "terms", href: "/terms" },
  ],
};

export const FOOTER_COLUMNS: readonly FooterColumn[] = [
  PRODUCT_COLUMN,
  COMPANY_COLUMN,
] as const;

/** `label` is the visible text: a brand name, not translated. */
export type FooterSocial = {
  key: string;
  label: string;
  href: string;
};

export const FOOTER_SOCIALS: readonly FooterSocial[] = [
  { key: "x", label: "X", href: "https://x.com/AWSD_JV" },
  {
    key: "github",
    label: "GitHub",
    href: "https://github.com/sicotjeanvivien/yog-sothoth",
  },
] as const;

export const FOOTER_AWSD = {
  href: "https://awsd.fr/",
} as const;
