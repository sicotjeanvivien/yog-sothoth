import { describe, expect, it } from "vitest";

import {
  parseTab,
  PROTOCOL_TABS,
  readyTabs,
  tabForKey,
  tabNumber,
  type ProtocolTab,
} from "../tabs";

const ready = (...tabs: ProtocolTab[]) => new Set<ProtocolTab>(tabs);

describe("parseTab", () => {
  it("keeps a ready tab asked for", () => {
    expect(parseTab("fees", ready("liquidity", "fees"))).toBe("fees");
  });

  // Mutation: a tab not shipped yet reachable by its URL, an empty panel under the row.
  it("falls back to the first ready tab when the one asked for has not shipped", () => {
    expect(parseTab("volume", ready("liquidity", "fees"))).toBe("liquidity");
  });

  it("falls back on a missing, unknown or repeated value", () => {
    const shipped = ready("fees", "liquidity");
    expect(parseTab(undefined, shipped)).toBe("liquidity");
    expect(parseTab("nope", shipped)).toBe("liquidity");
    expect(parseTab(["fees", "fees"], shipped)).toBe("liquidity");
  });

  it("is null while no tab has shipped", () => {
    expect(parseTab("liquidity", ready())).toBeNull();
  });
});

describe("readyTabs", () => {
  it("lists the ready tabs in the row's order, not the set's", () => {
    expect(readyTabs(ready("tokens", "liquidity"))).toEqual(["liquidity", "tokens"]);
  });
});

describe("tabNumber", () => {
  // Mutation: numbered among the ready tabs, a shipped tab would change number as others arrive.
  it("is the tab's place in the full list", () => {
    expect(tabNumber("liquidity")).toBe("01");
    expect(tabNumber("fees")).toBe("03");
    expect(tabNumber("activity")).toBe("06");
  });

  it("covers the six tabs of the canvas", () => {
    expect(PROTOCOL_TABS).toHaveLength(6);
  });
});

describe("tabForKey", () => {
  it("opens the ready tab of that number", () => {
    expect(tabForKey("3", ready("liquidity", "fees"))).toBe("fees");
  });

  // Mutation: a shortcut that opens a tab not shipped yet.
  it("does nothing on the number of a tab not shipped", () => {
    expect(tabForKey("2", ready("liquidity", "fees"))).toBeNull();
  });

  it("does nothing on any other key", () => {
    const shipped = ready(...PROTOCOL_TABS);
    for (const key of ["0", "7", "12", "a", "Enter"]) {
      expect(tabForKey(key, shipped)).toBeNull();
    }
  });
});
