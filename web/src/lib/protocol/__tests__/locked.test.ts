import { describe, expect, it } from "vitest";

import { lockedTokensOf } from "../locked";

const token = (symbol: string) => ({
  decimals: 9,
  logoUri: null,
  mint: `${symbol}mint`,
  name: null,
  symbol,
});

const page = (rows: [string, string | null][], total: string | null) => ({
  items: rows.map(([symbol, lockedUsd]) => ({ token: token(symbol), lockedUsd })),
  total: { lockedUsd: total },
});

describe("lockedTokensOf", () => {
  // Mutation: the rest summed from the rows only, or the rows' sum not taken from the total.
  it("gives the rest of the reading to the other tokens", () => {
    const { tokens, others } = lockedTokensOf(
      page(
        [
          ["SOL", "40"],
          ["USDC", "30"],
          ["MET", "10"],
          ["JUP", "5"],
        ],
        "100",
      ),
    );

    expect(tokens.map((t) => t.usd)).toEqual([40, 30, 10, 5]);
    expect(others).toEqual({ usd: 15, share: 0.15 });
  });

  // Mutation: a share taken over the rows' sum, not the reading's total.
  it("shares each token over the reading's total", () => {
    const { tokens } = lockedTokensOf(page([["SOL", "25"]], "100"));

    expect(tokens[0]!.share).toBe(0.25);
  });

  // Mutation: an unknown row counted as 0, which hands its part to the others.
  it("leaves the rest unknown when a row is", () => {
    const { tokens, others } = lockedTokensOf(
      page(
        [
          ["SOL", "40"],
          ["MET", null],
        ],
        "100",
      ),
    );

    expect(tokens[1]).toMatchObject({ usd: null, share: null });
    expect(others).toBeNull();
  });

  // Mutation: an unknown total replaced by the rows' sum, which shares them over themselves.
  it("has no shares and no rest without a total", () => {
    const { tokens, others } = lockedTokensOf(page([["SOL", "40"]], null));

    expect(tokens[0]!.share).toBeNull();
    expect(others).toBeNull();
  });

  // Mutation: a rest below a cent kept, drawn as « Others » at $0.
  it("shows no rest under a cent", () => {
    expect(lockedTokensOf(page([["SOL", "100"]], "100.004")).others).toBeNull();
  });
});
