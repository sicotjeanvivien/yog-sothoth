import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";

import { describe, expect, it } from "vitest";

const SRC = join(__dirname, "..", "..");
const FONTS = join(SRC, "app", "fonts");
const LAYOUT = readFileSync(join(SRC, "app", "[locale]", "layout.tsx"), "utf8");

function files(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    return statSync(path).isDirectory() ? files(path) : [path];
  });
}

describe("fonts", () => {
  // Mutation: `next/font/google` back in the layout — the build downloads from Google again.
  it("are never fetched from Google at build time", () => {
    const remote = files(SRC).filter(
      (path) =>
        /\.(ts|tsx|js|jsx|mjs|css)$/.test(path) &&
        !path.includes("__tests__") &&
        readFileSync(path, "utf8").includes("next/font/google"),
    );
    expect(remote.map((path) => relative(SRC, path))).toEqual([]);
  });

  // Mutation: one family's latin range edited — its latin and latin-ext faces overlap or leave a gap.
  it("give the three latin faces the same range", () => {
    const ranges = [...LAYOUT.matchAll(/prop: "unicode-range", value: "([^"]+)"/g)].map((m) => m[1] ?? "");

    expect(ranges).toHaveLength(3);
    expect(new Set(ranges).size).toBe(1);
  });

  // Mutation: a family named apart from its const — the CSS variable points at a face that does not exist.
  it("are each named after their const, in the layout and in fonts.css", () => {
    const declared = [
      ...LAYOUT.matchAll(/const (\w+) = localFont\(\{[^]*?prop: "font-family", value: "([^"]+)"/g),
    ].map((m) => [m[1], m[2]]);
    const css = readFileSync(join(FONTS, "fonts.css"), "utf8");
    const subsets = new Set([...css.matchAll(/font-family: '([^']+)'/g)].map((m) => m[1]));

    expect(declared).toEqual([
      ["cinzel", "cinzel"],
      ["geist", "geist"],
      ["geistMono", "geistMono"],
    ]);
    expect([...subsets].sort()).toEqual(["cinzel", "geist", "geistMono"]);
  });

  // Mutation: a subset file renamed or left out — its characters fall back to the system font.
  it("point at files that exist, every subset of each family", () => {
    const css = readFileSync(join(FONTS, "fonts.css"), "utf8");
    const urls = [...css.matchAll(/url\('\.\/([^']+)'\)/g)].map((m) => m[1] ?? "");

    expect(urls.filter((url) => !existsSync(join(FONTS, url)))).toEqual([]);
    expect(urls.sort()).toEqual([
      "cinzel-latin-ext.woff2",
      ...["geist", "geist-mono"].flatMap((family) =>
        ["cyrillic", "cyrillic-ext", "latin-ext", "vietnamese"].map((subset) => `${family}-${subset}.woff2`),
      ),
    ].sort());
  });
});
