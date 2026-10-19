// The public site: its origin, the pages a search engine may index, and the preview image.

/** The canonical origin, whatever host serves the page: canonical and sitemap URLs point here. */
export const SITE_URL = "https://yog-scope.xyz";

export const SITE_NAME = "Yog-Scope";

/**
 * Every indexable page, by its path after the locale (`""` is the home page). The sitemap and
 * the page guard read this list: a page added under `app/[locale]/` is added here.
 */
export const INDEXABLE_PATHS = [
  "",
  "/about",
  "/changelog",
  "/support-us",
  "/privacy",
  "/legal-notice",
  "/terms",
  "/overview",
  "/signals",
] as const;

export type IndexablePath = (typeof INDEXABLE_PATHS)[number];

/** One preview for every page and both languages, in `public/`. */
export const PREVIEW_IMAGE = {
  url: "/og-image.png",
  width: 1200,
  height: 630,
  alt: "Yog-Scope — Solana",
} as const;
