import { describe, expect, it } from "vitest";
import {
  poolRowSchema,
  poolSchema,
  protocolSchema,
  toPool,
  type PoolRow,
} from "../schemas";

describe("protocolSchema", () => {
  it("accepts every known protocol", () => {
    expect(protocolSchema.parse("damm_v2")).toBe("damm_v2");
    expect(protocolSchema.parse("damm_v1")).toBe("damm_v1");
    expect(protocolSchema.parse("dlmm")).toBe("dlmm");
  });

  it("rejects an unknown protocol", () => {
    expect(() => protocolSchema.parse("raydium")).toThrow();
  });
});

describe("poolRowSchema", () => {
  const validRow: PoolRow = {
    pool_address: "CGPxT5d1uf9a8cKVJuZaJAU76t2EfLGbTmRbfvLLZp5j",
    protocol: "damm_v2",
    token_a_mint: "So11111111111111111111111111111111111111112",
    token_b_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
    first_seen_at: new Date("2026-04-01T12:00:00Z"),
    last_seen_at: new Date("2026-04-30T18:30:00Z"),
  };

  it("accepts a valid row", () => {
    expect(() => poolRowSchema.parse(validRow)).not.toThrow();
  });

  it("rejects an empty pool_address", () => {
    expect(() =>
      poolRowSchema.parse({ ...validRow, pool_address: "" }),
    ).toThrow();
  });

  it("rejects a non-Date timestamp", () => {
    expect(() =>
      poolRowSchema.parse({
        ...validRow,
        last_seen_at: "2026-04-30T18:30:00Z",
      }),
    ).toThrow();
  });

  it("rejects an unknown protocol", () => {
    expect(() =>
      poolRowSchema.parse({ ...validRow, protocol: "raydium" }),
    ).toThrow();
  });
});

describe("toPool", () => {
  it("converts snake_case rows to camelCase API objects", () => {
    const row: PoolRow = {
      pool_address: "CGPxT5d1uf9a8cKVJuZaJAU76t2EfLGbTmRbfvLLZp5j",
      protocol: "damm_v2",
      token_a_mint: "So11111111111111111111111111111111111111112",
      token_b_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
      first_seen_at: new Date("2026-04-01T12:00:00.000Z"),
      last_seen_at: new Date("2026-04-30T18:30:00.000Z"),
    };

    const pool = toPool(row);

    expect(pool).toEqual({
      poolAddress: "CGPxT5d1uf9a8cKVJuZaJAU76t2EfLGbTmRbfvLLZp5j",
      protocol: "damm_v2",
      tokenAMint: "So11111111111111111111111111111111111111112",
      tokenBMint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
      firstSeenAt: "2026-04-01T12:00:00.000Z",
      lastSeenAt: "2026-04-30T18:30:00.000Z",
    });
  });

  it("emits ISO 8601 datetime strings that satisfy poolSchema", () => {
    const row: PoolRow = {
      pool_address: "abc",
      protocol: "damm_v2",
      token_a_mint: "mintA",
      token_b_mint: "mintB",
      first_seen_at: new Date("2026-01-01T00:00:00Z"),
      last_seen_at: new Date("2026-01-02T00:00:00Z"),
    };

    const pool = toPool(row);
    expect(() => poolSchema.parse(pool)).not.toThrow();
  });
});