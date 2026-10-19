/**
 * The metadata of one public page: title, description, preview and its two languages. ⚠️ Next
 * merges metadata shallowly: a page that set `title` alone would keep the layout's `og:title`.
 */

import type { Metadata } from "next";

import { routing } from "@/i18n/routing";
import { PREVIEW_IMAGE, SITE_NAME, type IndexablePath } from "@/config/site";

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
    alternates: {
      canonical: url,
      languages: {
        ...Object.fromEntries(routing.locales.map((l) => [l, `/${l}${path}`])),
        // The unprefixed path negotiates the language, then redirects: the page that chooses.
        "x-default": path === "" ? "/" : path,
      },
    },
    openGraph: {
      type: "website",
      siteName: SITE_NAME,
      locale,
      url,
      title,
      description,
      images: [PREVIEW_IMAGE],
    },
    twitter: {
      card: "summary_large_image",
      title,
      description,
      images: [PREVIEW_IMAGE.url],
    },
  };
}
