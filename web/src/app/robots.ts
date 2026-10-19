/** Every public page may be crawled; the sitemap says which to index. */

import type { MetadataRoute } from "next";

import { SITE_URL } from "@/config/site";

export default function robots(): MetadataRoute.Robots {
  return {
    rules: { userAgent: "*", allow: "/" },
    sitemap: `${SITE_URL}/sitemap.xml`,
  };
}
