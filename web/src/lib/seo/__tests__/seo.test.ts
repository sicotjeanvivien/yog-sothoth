import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative, sep } from "node:path";

import { describe, expect, it } from "vitest";

import sitemap from "@/app/sitemap";
import { INDEXABLE_PATHS, PREVIEW_IMAGE, SITE_URL } from "@/config/site";
import { pageMetadata } from "@/lib/seo/page-metadata";

const WEB = process.cwd();
const PAGES = join(WEB, "src", "app", "[locale]");

function files(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    return statSync(path).isDirectory() ? files(path) : [path];
  });
}

/** `src/app/[locale]/(marketing)/about/page.tsx` → `/about`; the home page → `""`. */
function routeOf(page: string): string {
  const segments = relative(PAGES, page)
    .split(sep)
    .slice(0, -1)
    .filter((segment) => !segment.startsWith("("));
  return segments.map((segment) => `/${segment}`).join("");
}

const pages = files(PAGES).filter((path) => path.endsWith(`${sep}page.tsx`));

describe("the indexable pages", () => {
  // Mutation: a page added under `app/[locale]/` without its path in `INDEXABLE_PATHS`.
  it("are exactly the pages under app/[locale]", () => {
    expect(pages.map(routeOf).sort()).toEqual([...INDEXABLE_PATHS].sort());
  });

  // Mutation: a page whose metadata is built by hand, without its canonical and languages.
  it("each build their metadata through pageMetadata", () => {
    const bare = pages.filter((page) => !readFileSync(page, "utf8").includes("pageMetadata("));
    expect(bare.map((page) => relative(WEB, page))).toEqual([]);
  });
});

describe("sitemap", () => {
  // Mutation: a path of the list left out of the sitemap, or one language missing.
  it("lists every indexable page in both languages, each naming the other", () => {
    const entries = sitemap();
    for (const path of INDEXABLE_PATHS) {
      for (const locale of ["en", "fr"]) {
        const entry = entries.find((e) => e.url === `${SITE_URL}/${locale}${path}`);
        expect(entry, `${locale}${path}`).toBeDefined();
        expect(entry?.alternates?.languages).toEqual({
          en: `${SITE_URL}/en${path}`,
          fr: `${SITE_URL}/fr${path}`,
        });
      }
    }
    expect(entries).toHaveLength(INDEXABLE_PATHS.length * 2);
  });
});

describe("pageMetadata", () => {
  const metadata = pageMetadata({
    locale: "en",
    path: "/signals",
    title: "Signals",
    description: "The feed.",
  });

  // Mutation: the canonical built from the request URL, query string included.
  it("names a canonical without parameters and both languages, x-default unprefixed", () => {
    expect(metadata.alternates).toEqual({
      canonical: "/en/signals",
      languages: { en: "/en/signals", fr: "/fr/signals", "x-default": "/signals" },
    });
  });

  // Mutation: `og:title` carrying the brand, which `og:site_name` already says.
  it("gives the preview the page's own title and the shared image", () => {
    expect(metadata.openGraph).toMatchObject({
      title: "Signals",
      siteName: "Yog-Scope",
      images: [PREVIEW_IMAGE],
    });
    expect(metadata.twitter).toMatchObject({ card: "summary_large_image" });
  });

  it("sends the home page's x-default to the root", () => {
    const home = pageMetadata({ locale: "fr", path: "", title: "t", description: "d" });
    expect(home.alternates?.languages).toMatchObject({ "x-default": "/" });
  });
});

describe("the title suffix", () => {
  // Mutation: a page or a message writing « — Yog-Scope » itself, beside the layout's template.
  it("is written once, in the layout's template", () => {
    const sources = [...files(join(WEB, "messages")), ...files(join(WEB, "src"))].filter(
      (path) => !path.includes(`${sep}__tests__${sep}`),
    );
    const writers = sources.filter((path) => readFileSync(path, "utf8").includes("— Yog-Scope"));
    expect(writers.map((path) => relative(WEB, path))).toEqual([
      join("src", "app", "[locale]", "layout.tsx"),
    ]);
  });
});
