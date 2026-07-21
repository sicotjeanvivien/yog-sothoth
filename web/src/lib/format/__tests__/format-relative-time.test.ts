import { describe, expect, it } from "vitest";

import { formatRelativeTime } from "../format-relative-time";

// Fixed reference point so the tests are deterministic.
const NOW = new Date("2026-07-21T12:00:00Z");
const TWO_HOURS_AGO = "2026-07-21T10:00:00Z";

describe("formatRelativeTime", () => {
  it("returns the em-dash for an invalid timestamp", () => {
    expect(formatRelativeTime("not-a-date", "en", { now: NOW })).toBe("—");
  });

  it("defaults to the long style", () => {
    expect(formatRelativeTime(TWO_HOURS_AGO, "en", { now: NOW })).toBe(
      "2 hours ago",
    );
  });

  it("short style is compact and never longer than long", () => {
    const long = formatRelativeTime(TWO_HOURS_AGO, "en", { now: NOW });
    const short = formatRelativeTime(TWO_HOURS_AGO, "en", {
      now: NOW,
      style: "short",
    });
    expect(short.length).toBeLessThan(long.length);
    expect(short).toContain("2");
  });

  it("stays locale-aware in short style (keeps 'il y a' in French)", () => {
    const shortFr = formatRelativeTime(TWO_HOURS_AGO, "fr", {
      now: NOW,
      style: "short",
    });
    expect(shortFr.toLowerCase()).toContain("il y a");
  });
});
