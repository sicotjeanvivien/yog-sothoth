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

/** A value of the server's `.env`, where the compose interpolates one (`${YOG_API_KEY:?…}`). */
const FROM_DOT_ENV: Record<string, string> = {
  YOG_API_KEY: `yog_01020304_${"ab".repeat(32)}`,
};

/** `environment` as the container receives it, an interpolated value read from the `.env`. */
function resolved(environment: Record<string, string>): Record<string, string> {
  return Object.fromEntries(
    Object.entries(environment).map(([name, value]) => [
      name,
      value.startsWith("${") ? (FROM_DOT_ENV[name] ?? value) : value,
    ]),
  );
}

function webService(): WebService {
  const compose = parse(readRepoFile("docker-compose.yml"));
  return compose.services["yog-web"];
}

describe("docker-compose.yml — the yog-web service", () => {
  it("gives the server every variable its schema requires, under that name", () => {
    const environment = resolved(webService().environment ?? {});

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
