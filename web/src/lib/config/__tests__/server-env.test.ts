/** `parseServerEnv`, from its argument rather than `process.env`. */

import { describe, expect, it } from "vitest";
import { parseServerEnv } from "../server-env.schema";

const KEY = `yog_0badc0de_${"a".repeat(64)}`;

function validInput(): Record<string, string | undefined> {
  return {
    YOG_API_URL: "http://127.0.0.1:3001",
    YOG_API_KEY: KEY,
  };
}

describe("parseServerEnv — happy path", () => {
  it("parses a minimal valid input and applies the timeout default", () => {
    const env = parseServerEnv(validInput());

    expect(env.YOG_API_URL).toBe("http://127.0.0.1:3001");
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

  it("accepts an https URL", () => {
    const env = parseServerEnv({
      ...validInput(),
      YOG_API_URL: "https://api.yog-scope.xyz",
    });

    expect(env.YOG_API_URL).toBe("https://api.yog-scope.xyz");
  });
});

describe("parseServerEnv — failure modes", () => {
  it("rejects a missing base URL", () => {
    expect(() => parseServerEnv({ YOG_API_KEY: KEY })).toThrow(/YOG_API_URL/);
  });

  it("rejects a base URL that is not a URL", () => {
    expect(() =>
      parseServerEnv({ ...validInput(), YOG_API_URL: "not-a-url" }),
    ).toThrow(/YOG_API_URL must be a valid URL/);
  });

  it("rejects a base URL that ends with a trailing slash", () => {
    expect(() =>
      parseServerEnv({ ...validInput(), YOG_API_URL: "http://127.0.0.1:3001/" }),
    ).toThrow(/trailing slash/);
  });

  it("rejects a missing key, naming the command that creates one", () => {
    expect(() =>
      parseServerEnv({ YOG_API_URL: "http://127.0.0.1:3001" }),
    ).toThrow(/YOG_API_KEY is required: create it with `yog-migrate create-api-key`/);
  });

  // Mutations: the key's pattern dropped, or loosened to any length or case.
  it.each([
    ["empty", ""],
    ["a ticket", `yogt_${"a".repeat(64)}`],
    ["a truncated secret", `yog_0badc0de_${"a".repeat(63)}`],
    ["a short prefix", `yog_0badc0d_${"a".repeat(64)}`],
    ["upper-case hex", `yog_0BADC0DE_${"a".repeat(64)}`],
    ["a trailing newline", `${KEY}\n`],
  ])("rejects a key that is %s", (_, key) => {
    expect(() => parseServerEnv({ ...validInput(), YOG_API_KEY: key })).toThrow(
      /YOG_API_KEY must read yog_<8 hex>_<64 hex>/,
    );
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
        YOG_API_URL: "not-a-url",
        YOG_API_TIMEOUT_MS: "-1",
      });
    } catch (e) {
      caught = e;
    }

    expect(caught).toBeInstanceOf(Error);
    const message = (caught as Error).message;
    expect(message).toMatch(/YOG_API_URL/);
    expect(message).toMatch(/YOG_API_KEY/);
    expect(message).toMatch(/YOG_API_TIMEOUT_MS/);
  });
});