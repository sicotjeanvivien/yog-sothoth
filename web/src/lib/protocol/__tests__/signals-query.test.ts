import { describe, expect, it } from "vitest";

import { periodKeep, readSignalsQuery, signalsQueryParams } from "../signals-query";

describe("readSignalsQuery", () => {
  it("reads a filtered page and its direction", () => {
    expect(readSignalsQuery({ severity: "critical", cursor: "abc", dir: "prev" })).toEqual({
      severity: "critical",
      page: { cursor: "abc", dir: "prev" },
    });
  });

  // Mutation: an unknown severity passed through makes the API answer 400, an error block.
  it("reads a missing, unknown or repeated severity as none", () => {
    expect(readSignalsQuery({}).severity).toBeNull();
    expect(readSignalsQuery({ severity: "major" }).severity).toBeNull();
    expect(readSignalsQuery({ severity: ["info", "info"] }).severity).toBeNull();
  });

  // Mutation: an empty or repeated cursor sent as is, the API refuses it.
  it("reads an empty or repeated cursor as the first page", () => {
    expect(readSignalsQuery({ cursor: "" }).page).toBeNull();
    expect(readSignalsQuery({ cursor: ["a", "b"] }).page).toBeNull();
  });

  // Mutation: a direction without a cursor reads as a page.
  it("ignores a direction without a cursor", () => {
    expect(readSignalsQuery({ dir: "prev" }).page).toBeNull();
  });

  // Mutation: an unknown direction kept, the API refuses it.
  it("reads an unknown direction as the default, older", () => {
    expect(readSignalsQuery({ cursor: "abc", dir: "up" }).page).toEqual({ cursor: "abc", dir: "next" });
  });
});

describe("signalsQueryParams", () => {
  // Mutation: `dir=next` written out gives one page two URLs.
  it("leaves the default direction and an empty filter out", () => {
    expect(signalsQueryParams({ severity: null, page: { cursor: "abc", dir: "next" } })).toEqual({
      cursor: "abc",
    });
    expect(signalsQueryParams({ severity: null, page: null })).toEqual({});
  });

  it("writes the filter and a step back", () => {
    expect(
      signalsQueryParams({ severity: "warning", page: { cursor: "abc", dir: "prev" } }),
    ).toEqual({ severity: "warning", cursor: "abc", dir: "prev" });
  });
});

describe("periodKeep", () => {
  const place = { severity: "info", page: { cursor: "abc", dir: "prev" } } as const;

  // Mutation: the period link keeping the tab only, a period change sends the list to its first page.
  it("keeps the Signals tab's filter and page", () => {
    expect(periodKeep("signals", place)).toEqual({
      tab: "signals",
      severity: "info",
      cursor: "abc",
      dir: "prev",
    });
  });

  // Mutation: the signals' place kept on every tab, another tab's URL carries it.
  it("keeps only the tab elsewhere", () => {
    expect(periodKeep("fees", place)).toEqual({ tab: "fees" });
  });
});
