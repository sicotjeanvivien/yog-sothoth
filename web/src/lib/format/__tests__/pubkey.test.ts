import { describe, expect, it } from "vitest";
import { shortenPubkey } from "../pubkey";

describe("shortenPubkey", () => {
  it("shortens a standard 44-char Solana pubkey", () => {
    const full = "CGPxT5d1uf9a8cKVJuZaJAU76t2EfLGbTmRbfvLLZp5j";
    expect(shortenPubkey(full)).toBe("CGPx…Zp5j");
  });

  it("honours a custom char count", () => {
    const full = "CGPxT5d1uf9a8cKVJuZaJAU76t2EfLGbTmRbfvLLZp5j";
    expect(shortenPubkey(full, 6)).toBe("CGPxT5…LLZp5j");
  });

  it("returns the input unchanged when it is already short", () => {
    expect(shortenPubkey("abcd")).toBe("abcd");
  });

  it("returns the input unchanged at the threshold", () => {
    // 4 + 1 (ellipsis) + 4 = 9. An input of 9 chars wouldn't benefit
    // from shortening, so it must pass through.
    expect(shortenPubkey("abcdefghi")).toBe("abcdefghi");
  });

  it("rejects a non-positive chars argument", () => {
    expect(() => shortenPubkey("anything", 0)).toThrow(RangeError);
    expect(() => shortenPubkey("anything", -1)).toThrow(RangeError);
  });
});