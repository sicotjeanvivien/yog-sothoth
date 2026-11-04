import { describe, expect, it } from "vitest";

import { ACTIVITY_TOPICS, livePath, pageTopics } from "../live-topics";

describe("page topics", () => {
  // Mutation: a collection left out — its events never reach the activity table.
  it("adds every on-chain collection but the swaps in the advanced mode", () => {
    expect(pageTopics(false)).toEqual(["signals"]);
    expect(pageTopics(true)).toEqual([
      "signals",
      "liquidity-changes",
      "pool-creations",
      "position-changes",
      "position-fee-claims",
      "protocol-fee-claims",
      "reward-claims",
      "reward-changes",
      "pool-changes",
    ]);
    expect(ACTIVITY_TOPICS).not.toContain("swaps");
  });
});

// Mutation: a protocol's stream opened without it — tab 06 would show every protocol's events.
describe("live path", () => {
  it("carries every topic of the page on one connection", () => {
    expect(livePath(pageTopics(true))).toBe(
      "/api/live?topics=signals,liquidity-changes,pool-creations,position-changes," +
        "position-fee-claims,protocol-fee-claims,reward-claims,reward-changes,pool-changes",
    );
  });

  it("opens one protocol's stream under its scope", () => {
    expect(livePath(["signals"], "meteora_damm_v2")).toBe(
      "/api/live/protocols/meteora_damm_v2?topics=signals",
    );
  });
});
