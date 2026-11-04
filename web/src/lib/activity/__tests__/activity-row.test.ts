import { describe, expect, it } from "vitest";

import { activityRow, toActivityItem, type ActivityItem } from "../activity-row";
import { BASE, MET, OWNER, POSITION, SOL, UNKNOWN } from "./fixtures";

function liquidity(kind: "add" | "remove", tokens = { tokenA: MET, tokenB: SOL }): ActivityItem {
  return {
    topic: "liquidity-changes",
    event: {
      ...BASE,
      ...tokens,
      kind,
      amountA: "2500000",
      amountB: "1480353083770",
      reserveAAfter: "1",
      reserveBAfter: "1",
      position: POSITION,
      owner: OWNER,
    },
  };
}

describe("activityRow", () => {
  // Mutation: A and B exchanged, or the raw amount shown — 2.5 MET read as 1.48K.
  it("gives a liquidity change its pair, signed amounts in units of each side, and its position", () => {
    const row = activityRow(liquidity("add"));
    expect(row.kind).toBe("liquidity_add");
    expect(row.pool).toBe("⁨MET⁩ / ⁨SOL⁩");
    expect(row.detail).toEqual([
      { type: "amount", sign: "+", value: "2.5", unit: "MET" },
      { type: "amount", sign: "+", value: "1.48K", unit: "SOL" },
      { type: "party", role: "position", address: "9fQ2…L8xa" },
    ]);
    expect(activityRow(liquidity("remove")).detail[0]).toMatchObject({ sign: "−" });
  });

  // Mutation: a fallback to 0 decimals — the raw amount passes for units of the token.
  it("shows no amount of a side whose decimals are unknown, and the pool by address", () => {
    const row = activityRow(liquidity("add", { tokenA: UNKNOWN, tokenB: SOL }));
    expect(row.pool).toBe("ELR1…9xQz");
    expect(row.detail).toEqual([
      { type: "amount", sign: "+", value: "1.48K", unit: "SOL" },
      { type: "party", role: "position", address: "9fQ2…L8xa" },
    ]);
  });

  // Mutation: a reward amount read with a side's decimals although its mint is neither.
  it("shows a reward's amount only when its mint is one side of the pair", () => {
    const claim = (rewardMint: string): ActivityItem => ({
      topic: "reward-claims",
      event: { ...BASE, amount: "2500000", rewardMint, rewardIndex: 0, position: POSITION, owner: OWNER },
    });
    expect(activityRow(claim(MET.mint as string)).detail).toEqual([
      { type: "amount", sign: "", value: "2.5", unit: "MET" },
      { type: "party", role: "owner", address: "5hTr…Wq1c" },
    ]);
    expect(activityRow(claim(UNKNOWN.mint as string)).detail).toEqual([
      { type: "party", role: "owner", address: "5hTr…Wq1c" },
    ]);
  });

  it("names each kind of change by its own block, and a protocol's change without one", () => {
    const position = (meteoraDammV2: unknown) =>
      activityRow({ topic: "position-changes", event: { ...BASE, meteoraDammV2 } } as ActivityItem).kind;
    expect(position({ kind: "open", owner: OWNER, position: POSITION, positionNftMint: OWNER })).toBe(
      "position_open",
    );
    expect(position({ kind: "close", owner: OWNER, position: POSITION, positionNftMint: OWNER })).toBe(
      "position_close",
    );
    expect(position(undefined)).toBe("position_change");

    const pool = (meteoraDammV2: unknown) =>
      activityRow({ topic: "pool-changes", event: { ...BASE, meteoraDammV2 } } as ActivityItem).kind;
    expect(pool({ kind: "status", status: 1 })).toBe("pool_status");
    expect(pool(undefined)).toBe("pool_change");

    expect(
      activityRow({ topic: "pool-creations", event: { ...BASE, tokenAMint: "a", tokenBMint: "b", creator: OWNER } })
        .detail,
    ).toEqual([{ type: "party", role: "creator", address: "5hTr…Wq1c" }]);
  });

  // Mutation: the key without its topic — two collections' events of one instruction collapse.
  it("keys a row by its topic, signature and index", () => {
    expect(activityRow(liquidity("add")).key).toBe(`liquidity-changes:${BASE.signature}:0`);
  });
});

describe("toActivityItem", () => {
  // Mutation: a message taken without its collection's schema — a malformed one reaches the table.
  it("keeps a message only as its collection's item", () => {
    expect(toActivityItem("liquidity-changes", liquidity("add").event)).not.toBeNull();
    expect(toActivityItem("pool-creations", liquidity("add").event)).toBeNull();
    expect(toActivityItem("liquidity-changes", { at: "nope" })).toBeNull();
  });
});
