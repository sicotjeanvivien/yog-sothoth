/**
 * The metadata of one public page: title, description, preview and its two languages. ⚠️ Next
 * merges metadata shallowly: a page that set `title` alone would keep the layout's `og:title`.
 */

import type { Metadata } from "next";

import { routing } from "@/i18n/routing";
import { PREVIEW_IMAGE, SITE_NAME, type IndexablePath } from "@/config/site";

/** Open Graph names a locale `language_TERRITORY`; a bare `fr` is read as `en_US`. */
const OG_LOCALES: Record<string, string> = { en: "en_US", fr: "fr_FR" };

/** The page in each language, and `x-default` to the unprefixed path, which negotiates one. */
export function localizedAlternates(path: IndexablePath): Record<string, string> {
  return {
    ...Object.fromEntries(routing.locales.map((l) => [l, `/${l}${path}`])),
    "x-default": path === "" ? "/" : path,
  };
}

type PageMetadataInput = {
  locale: string;
  path: IndexablePath;
  title: string;
  description: string;
  /** The home page names the brand itself, outside the layout's title template. */
  absoluteTitle?: boolean;
};

export function pageMetadata({
  locale,
  path,
  title,
  description,
  absoluteTitle = false,
}: PageMetadataInput): Metadata {
  const url = `/${locale}${path}`;
  return {
    title: absoluteTitle ? { absolute: title } : title,
    description,
    alternates: { canonical: url, languages: localizedAlternates(path) },
    openGraph: {
      type: "website",
      siteName: SITE_NAME,
      locale: OG_LOCALES[locale] ?? locale,
      alternateLocale: routing.locales
        .filter((l) => l !== locale)
        .map((l) => OG_LOCALES[l] ?? l),
      url,
      title,
      description,
      images: [PREVIEW_IMAGE],
    },
    twitter: {
      card: "summary_large_image",
      title,
      description,
      images: [PREVIEW_IMAGE],
    },
  };
}
