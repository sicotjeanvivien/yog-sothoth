/**
 * The `yog-web` service of the root `docker-compose.yml`, checked against
 * what the web app actually reads.
 *
 * The compose file and the web app name the same variables from two sides,
 * and nothing else compares them. A mismatch costs nothing at build time
 * and everything at run time: the compose once passed `API_INTERNAL_URL`
 * while the server reads `YOG_API_INTERNAL_URL`, and every data page of
 * `--profile full` answered 500.
 *
 * The server case runs the real schema on the real values, so it needs no
 * copied list of names to keep in sync. The build-argument case exists
 * because Docker ignores an argument the Dockerfile does not declare,
 * with a warning only — the browser's API URL would then silently not
 * reach `next build`.
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
