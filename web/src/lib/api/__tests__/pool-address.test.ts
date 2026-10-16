/**
 * `isValidPoolAddress` against yog-api's rule: a base58 string that decodes to 32 bytes.
 */

import { describe, expect, it } from "vitest";

import { isValidPoolAddress } from "../pool-address";

const REAL_43_CHARS = "116cRswGYgRiGHUhQRHttYkUYrKZTMSqDYKfFCwSGjA";
const REAL_44_CHARS = "121W8hbvmSxshhpDED9F9u1FJkHkX7VqzZpamuEsX7qk";

describe("isValidPoolAddress", () => {
  it.each([
    ["a real 43-character address", REAL_43_CHARS],
    ["a real 44-character address", REAL_44_CHARS],
    ["the all-zero key, leading ones only", "1".repeat(32)],
  ])("accepts %s", (_, address) => {
    expect(isValidPoolAddress(address)).toBe(true);
  });

  // Catches a check that stops at the regex: every case below matches it.
  it.each([
    ["a real address cut by one character", REAL_43_CHARS.slice(0, -1)],
    ["a real address cut by two characters", REAL_44_CHARS.slice(0, -2)],
    ["44 leading ones, a 44-byte key", "1".repeat(44)],
    ["44 times z, a 33-byte key", "z".repeat(44)],
    ["32 times z, a 24-byte key", "z".repeat(32)],
  ])("rejects %s", (_, address) => {
    expect(isValidPoolAddress(address)).toBe(false);
  });

  it.each([
    ["too short", "xyz"],
    ["outside the alphabet", "not-a-pool!"],
    ["with a zero, absent from base58", `0${REAL_43_CHARS.slice(1)}`],
  ])("rejects an address %s", (_, address) => {
    expect(isValidPoolAddress(address)).toBe(false);
  });
});
