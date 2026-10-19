// The public site: its origin, the pages a search engine may index, and the preview image.

/** The canonical origin, whatever host serves the page: canonical and sitemap URLs point here. */
export const SITE_URL = "https://yog-scope.xyz";

export const SITE_NAME = "Yog-Scope";

/**
 * The pages the sitemap lists, by their path after the locale (`""` is the home page). A page
 * added under `app/[locale]/` goes here or in `UNLISTED_PATHS`, or the page guard fails.
 */
export const SITEMAP_PATHS = [
  "",
  "/about",
  "/changelog",
  "/support-us",
  "/privacy",
  "/legal-notice",
  "/terms",
] as const;

/** Served with their metadata, out of the sitemap: their reads are live, slow, and fail with the API. */
export const UNLISTED_PATHS = ["/overview", "/signals"] as const;

export type PagePath = (typeof SITEMAP_PATHS)[number] | (typeof UNLISTED_PATHS)[number];

/** One preview for every page and both languages, in `public/`. */
export const PREVIEW_IMAGE = {
  url: "/og-image.png",
  width: 1200,
  height: 630,
  alt: "Yog-Scope — Solana",
} as const;
