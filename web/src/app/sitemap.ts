/** Every indexable page in both languages, each naming the same alternates as its HTML. */

import type { MetadataRoute } from "next";

import { routing } from "@/i18n/routing";
import { INDEXABLE_PATHS, SITE_URL } from "@/config/site";
import { localizedAlternates } from "@/lib/seo/page-metadata";

export default function sitemap(): MetadataRoute.Sitemap {
  return INDEXABLE_PATHS.flatMap((path) => {
    const languages = Object.fromEntries(
      Object.entries(localizedAlternates(path)).map(([lang, href]) => [
        lang,
        `${SITE_URL}${href}`,
      ]),
    );
    return routing.locales.map((locale) => ({
      url: `${SITE_URL}/${locale}${path}`,
      alternates: { languages },
    }));
  });
}
