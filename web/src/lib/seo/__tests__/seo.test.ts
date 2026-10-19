import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative, sep } from "node:path";

import { describe, expect, it } from "vitest";

import sitemap from "@/app/sitemap";
import { PREVIEW_IMAGE, SITE_URL, SITEMAP_PATHS, UNLISTED_PATHS } from "@/config/site";
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
  // Mutation: a page added under `app/[locale]/` in neither list, or in both.
  it("are exactly the pages under app/[locale], each in one list", () => {
    const listed = [...SITEMAP_PATHS, ...UNLISTED_PATHS];
    expect(new Set(listed).size).toBe(listed.length);
    expect(pages.map(routeOf).sort()).toEqual(listed.sort());
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
    for (const path of SITEMAP_PATHS) {
      for (const locale of ["en", "fr"]) {
        const entry = entries.find((e) => e.url === `${SITE_URL}/${locale}${path}`);
        expect(entry, `${locale}${path}`).toBeDefined();
        expect(entry?.alternates?.languages).toEqual({
          en: `${SITE_URL}/en${path}`,
          fr: `${SITE_URL}/fr${path}`,
          "x-default": `${SITE_URL}${path === "" ? "/" : path}`,
        });
      }
    }
    expect(entries).toHaveLength(SITEMAP_PATHS.length * 2);
  });

  // Mutation: the dashboard pages listed, which sends crawlers onto live, slow API reads.
  it("leaves out the unlisted pages", () => {
    const urls = sitemap().map((entry) => entry.url);
    for (const path of UNLISTED_PATHS) {
      expect(urls.filter((url) => url.endsWith(path))).toEqual([]);
    }
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
      locale: "en_US",
      alternateLocale: ["fr_FR"],
      images: [PREVIEW_IMAGE],
    });
    expect(metadata.twitter).toMatchObject({
      card: "summary_large_image",
      images: [PREVIEW_IMAGE],
    });
  });

  // Mutation: the preview's URL kept while its file is not in `public/`, or not 1200 × 630.
  it("points at an image that exists, at the size it declares", () => {
    const png = readFileSync(join(WEB, "public", PREVIEW_IMAGE.url));
    expect(png.subarray(1, 4).toString("ascii")).toBe("PNG");
    expect([png.readUInt32BE(16), png.readUInt32BE(20)]).toEqual([
      PREVIEW_IMAGE.width,
      PREVIEW_IMAGE.height,
    ]);
  });

  it("sends the home page's x-default to the root", () => {
    const home = pageMetadata({ locale: "fr", path: "", title: "t", description: "d" });
    expect(home.alternates?.languages).toMatchObject({ "x-default": "/" });
  });
});

describe("the title suffix", () => {
  // Mutation: a page or a message writing « — Yog-Scope » itself, beside the layout's template.
  it("is written by the layout's template alone, from SITE_NAME", () => {
    const sources = [...files(join(WEB, "messages")), ...files(join(WEB, "src"))].filter(
      (path) => !path.includes(`${sep}__tests__${sep}`),
    );
    const writers = sources.filter((path) => readFileSync(path, "utf8").includes("— Yog-Scope"));
    expect(writers.map((path) => relative(WEB, path))).toEqual([]);
    const layout = readFileSync(join(PAGES, "layout.tsx"), "utf8");
    expect(layout).toContain("template: `%s — ${SITE_NAME}`");
  });
});
