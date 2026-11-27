import { describe, expect, it } from "vitest";

import type { SignalResponse } from "@/lib/api/schema/zod.gen";

import { signalHeadline } from "../signal-headline";

function signal(symbolA: string): SignalResponse {
  const token = { mint: null, name: null, decimals: null, logoUri: null };
  return {
    id: 1,
    detector: "flow_imbalance",
    protocol: "meteora_damm_v2",
    poolAddress: "5Q544fKrFoe6tsEbD7S8EmxGTJYAKtTVhAW5Q5pge4j1",
    tokenA: { ...token, symbol: symbolA },
    tokenB: { ...token, symbol: "USDC" },
    severity: "warning",
    value: "0.75",
    threshold: "0.6",
    valueUnit: "raw",
    leansToward: null,
    message: null,
    triggeredAt: "2026-07-02T10:00:00Z",
  };
}

const t = (_key: string, values: { pair: string; value: string }) => values.pair;

describe("signalHeadline", () => {
  // Mutation: the symbol written as is — "\u202ETDSU" reads "USDT" in the headline.
  it("shows an invisible character of a symbol in the pair", () => {
    expect(signalHeadline(signal("\u202ETDSU"), "en", t)).toBe("\u2068⟨U+202E⟩TDSU\u2069 / \u2068USDC\u2069");
  });
});
