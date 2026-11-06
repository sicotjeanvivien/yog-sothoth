/**
 * `parseServerEnv`, from its argument rather than `process.env`. ⚠️ Imported from
 * `server-env.schema`: the `server-env` facade's `"server-only"` would crash vitest.
 */

import { describe, expect, it } from "vitest";
import { parseServerEnv } from "../server-env.schema";

const KEY = "yog_01020304_abababababababababababababababababababababababababababababababab";

function validInput(): Record<string, string | undefined> {
  return {
    YOG_API_KEY: KEY,
  };
}

describe("parseServerEnv — happy path", () => {
  it("parses a minimal valid input and applies the timeout default", () => {
    const env = parseServerEnv(validInput());

    expect(env.YOG_API_KEY).toBe(KEY);
    expect(env.YOG_API_TIMEOUT_MS).toBe(5000);
  });

  it("accepts a custom timeout coerced from a string", () => {
    const env = parseServerEnv({
      ...validInput(),
      YOG_API_TIMEOUT_MS: "12000",
    });

    expect(env.YOG_API_TIMEOUT_MS).toBe(12000);
  });

});

describe("parseServerEnv — failure modes", () => {
  it("rejects a missing key", () => {
    expect(() => parseServerEnv({})).toThrow(/YOG_API_KEY/);
  });

  // Mutation: any string taken as the key — a typo would only show as 401s at the first page.
  it("rejects a key that is not an API key", () => {
    expect(() => parseServerEnv({ YOG_API_KEY: "secret" })).toThrow(
      /YOG_API_KEY must be an API key/,
    );
    expect(() => parseServerEnv({ YOG_API_KEY: KEY.toUpperCase() })).toThrow(/YOG_API_KEY/);
  });

  it("rejects a negative timeout", () => {
    expect(() =>
      parseServerEnv({
        ...validInput(),
        YOG_API_TIMEOUT_MS: "-1",
      }),
    ).toThrow(/must be positive/);
  });

  it("rejects a non-integer timeout", () => {
    expect(() =>
      parseServerEnv({
        ...validInput(),
        YOG_API_TIMEOUT_MS: "12.5",
      }),
    ).toThrow(/must be an integer/);
  });

  it("rejects a non-numeric timeout", () => {
    expect(() =>
      parseServerEnv({
        ...validInput(),
        YOG_API_TIMEOUT_MS: "abc",
      }),
    ).toThrow();
  });

  it("aggregates multiple errors into a single throw", () => {
    let caught: unknown = null;
    try {
      parseServerEnv({
        YOG_API_KEY: "secret",
        YOG_API_TIMEOUT_MS: "-1",
      });
    } catch (e) {
      caught = e;
    }

    expect(caught).toBeInstanceOf(Error);
    const message = (caught as Error).message;
    expect(message).toMatch(/YOG_API_KEY/);
    expect(message).toMatch(/YOG_API_TIMEOUT_MS/);
  });
});