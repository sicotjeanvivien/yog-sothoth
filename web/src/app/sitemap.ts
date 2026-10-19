/** Every indexable page in both languages, each naming its other language. */

import type { MetadataRoute } from "next";

import { routing } from "@/i18n/routing";
import { INDEXABLE_PATHS, SITE_URL } from "@/config/site";

export default function sitemap(): MetadataRoute.Sitemap {
  return INDEXABLE_PATHS.flatMap((path) => {
    const languages = Object.fromEntries(
      routing.locales.map((locale) => [locale, `${SITE_URL}/${locale}${path}`]),
    );
    return routing.locales.map((locale) => ({
      url: `${SITE_URL}/${locale}${path}`,
      alternates: { languages },
    }));
  });
}
