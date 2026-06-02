/**
 * Tests for the server-only environment parser.
 *
 * Imports from `../server-env.schema` (not `../server-env`) on purpose:
 * the `.schema` module is bundler-neutral and runs fine under vitest.
 * The `.ts` facade has a `"server-only"` directive that would crash
 * the test runner (which has no Next.js bundler boundary to enforce).
 *
 * The unit under test is `parseServerEnv` because it takes its source
 * as an argument: this keeps the suite independent of `process.env`
 * and lets each case describe its own input explicitly.
 */

import { describe, expect, it } from "vitest";
import { parseServerEnv } from "../server-env.schema";

// Build a valid base input that each test mutates as needed. Keeps
// individual cases short and focused on the field they exercise.
function validInput(): Record<string, string | undefined> {
  return {
    YOG_API_INTERNAL_URL: "http://127.0.0.1:3001",
  };
}

describe("parseServerEnv — happy path", () => {
  it("parses a minimal valid input and applies the timeout default", () => {
    const env = parseServerEnv(validInput());

    expect(env.YOG_API_INTERNAL_URL).toBe("http://127.0.0.1:3001");
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
      YOG_API_INTERNAL_URL: "https://api.yog-sothoth.fr",
    });

    expect(env.YOG_API_INTERNAL_URL).toBe("https://api.yog-sothoth.fr");
  });
});

describe("parseServerEnv — failure modes", () => {
  it("rejects a missing base URL", () => {
    expect(() => parseServerEnv({})).toThrow(/YOG_API_INTERNAL_URL/);
  });

  it("rejects a base URL that is not a URL", () => {
    expect(() =>
      parseServerEnv({ YOG_API_INTERNAL_URL: "not-a-url" }),
    ).toThrow(/YOG_API_INTERNAL_URL must be a valid URL/);
  });

  it("rejects a base URL that ends with a trailing slash", () => {
    expect(() =>
      parseServerEnv({ YOG_API_INTERNAL_URL: "http://127.0.0.1:3001/" }),
    ).toThrow(/trailing slash/);
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
        YOG_API_INTERNAL_URL: "not-a-url",
        YOG_API_TIMEOUT_MS: "-1",
      });
    } catch (e) {
      caught = e;
    }

    expect(caught).toBeInstanceOf(Error);
    const message = (caught as Error).message;
    expect(message).toMatch(/YOG_API_INTERNAL_URL/);
    expect(message).toMatch(/YOG_API_TIMEOUT_MS/);
  });
});