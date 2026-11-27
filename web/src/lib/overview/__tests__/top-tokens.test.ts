import { describe, expect, it } from "vitest";

import { topTokenName, topTokenSymbol } from "../top-tokens";

const token = { mint: "CGPxT5d1uf9a8cKVJuZaJAU76t2EfLGbTmRbfvLLZp5j", decimals: 6, logoUri: null };

describe("topTokenSymbol", () => {
  // Mutation: the symbol written as is — "\u202ETDSU" reads "USDT" in the block.
  it("shows an invisible character of a symbol", () => {
    expect(topTokenSymbol({ ...token, symbol: "\u202ETDSU", name: null })).toBe("\u2068⟨U+202E⟩TDSU\u2069");
  });

  // Mutation: the fallbacks swapped, or a missing symbol shown as "null".
  it("names a token by its short mint while its symbol is unknown, by a dash without either", () => {
    expect(topTokenSymbol({ ...token, symbol: null, name: null })).toBe("CGPx...Zp5j");
    expect(topTokenSymbol({ ...token, mint: null, symbol: null, name: null })).toBe("—");
  });
});

describe("topTokenName", () => {
  // Mutation: the name written as is — a reversed name passes for another.
  it("shows an invisible character of a name", () => {
    expect(topTokenName({ ...token, symbol: "X", name: "\u202EtaC enoC" })).toBe("\u2068⟨U+202E⟩taC enoC\u2069");
  });

  // Mutation: an empty name kept — a blank span after the symbol.
  it("has no name when it is unknown or empty", () => {
    expect(topTokenName({ ...token, symbol: "X", name: null })).toBeNull();
    expect(topTokenName({ ...token, symbol: "X", name: "" })).toBeNull();
  });
});
