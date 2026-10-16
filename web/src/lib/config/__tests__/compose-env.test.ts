/**
 * The `yog-web` service of `docker-compose.yml` against what the web app reads: nothing else
 * compares them, and a mismatch breaks only at run time. ⚠️ Docker ignores a build argument
 * the Dockerfile does not declare, with a warning only.
 */

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { parse } from "yaml";
import { parseServerEnv } from "../server-env.schema";

function readRepoFile(relativeToRepoRoot: string): string {
  const url = new URL(`../../../../../${relativeToRepoRoot}`, import.meta.url);
  return readFileSync(fileURLToPath(url), "utf8");
}

interface WebService {
  environment?: Record<string, string>;
  build?: { args?: Record<string, string> };
}

function webService(): WebService {
  const compose = parse(readRepoFile("docker-compose.yml"));
  return compose.services["yog-web"];
}

describe("docker-compose.yml — the yog-web service", () => {
  it("gives the server every variable its schema requires, under that name", () => {
    const environment = webService().environment ?? {};

    expect(() => parseServerEnv(environment)).not.toThrow();
  });

  it("declares in web/Dockerfile every build argument it passes", () => {
    const passed = Object.keys(webService().build?.args ?? {});
    const declared = [
      ...readRepoFile("web/Dockerfile").matchAll(/^ARG\s+([A-Za-z0-9_]+)/gm),
    ].map((match) => match[1]);

    const undeclared = passed.filter((name) => !declared.includes(name));

    expect(passed).not.toHaveLength(0);
    expect(undeclared).toEqual([]);
  });
});
