import { readFileSync } from "node:fs";
import { join } from "node:path";

import { describe, expect, it } from "vitest";

import spec from "@/lib/api/schema/openapi.json";
import { RELEASES } from "@/components/marketing/changelog/releases";

import { openApiBody } from "../contract-file";

const committed = readFileSync(join(process.cwd(), "..", "crates", "api", "openapi.json"), "utf8");

describe("the contract the page serves", () => {
  // Mutation: the body stringified without its indent — a download that differs from the tag's.
  it("is the committed file, byte for byte", () => {
    expect(openApiBody()).toBe(committed);
  });

  // Mutation: a release in the changelog without the API crate's version raised to it.
  it("carries the version of the latest release", () => {
    expect(`v${spec.info.version}`).toBe(RELEASES[0]?.version);
  });
});
