import { describe, expect, it } from "vitest";

import { pairLabel, shortenPubkey } from "../pubkey";

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

describe("pairLabel", () => {
  // Mutation: a pair named with one symbol unknown — "SOL / null" in a cell; or a symbol not
  // isolated, so a right-to-left one reverses the line.
  it("names a pool by its pair, and by its address while a symbol is unknown", () => {
    const pool = { poolAddress: "CGPxRANDOMMIDDLEPARTZp5j", tokenASymbol: "SOL", tokenBSymbol: "USDC" };
    expect(pairLabel(pool)).toBe("\u2068SOL\u2069 / \u2068USDC\u2069");
    expect(pairLabel({ ...pool, tokenBSymbol: null })).toBe("CGPx…Zp5j");
  });

  // Mutation: the symbol written as is — "\u202ETDSU" reads "USDT" in the cell.
  it("shows an invisible character of a symbol", () => {
    const pool = { poolAddress: "CGPxRANDOMMIDDLEPARTZp5j", tokenASymbol: "\u202ETDSU", tokenBSymbol: "SOL" };
    expect(pairLabel(pool)).toBe("\u2068⟨U+202E⟩TDSU\u2069 / \u2068SOL\u2069");
  });
});
