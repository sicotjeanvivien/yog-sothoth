import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";

import { describe, expect, it } from "vitest";

const SRC = join(__dirname, "..", "..");

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
        /\.(ts|tsx|css)$/.test(path) &&
        !path.includes("__tests__") &&
        readFileSync(path, "utf8").includes("next/font/google"),
    );
    expect(remote.map((path) => relative(SRC, path))).toEqual([]);
  });
});
