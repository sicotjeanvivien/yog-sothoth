import { describe, expect, it } from "vitest";
import { formatAbsolute, formatRelative } from "../date";

// Reference "now" used across the suite — picks a Monday afternoon
// UTC so the daylight-saving edge does not surprise anyone reading
// the expectations.
const NOW = new Date("2026-05-12T15:00:00Z");

describe("formatRelative", () => {
  it("returns 'just now' below the one-minute threshold (en)", () => {
    const target = new Date(NOW.getTime() - 45 * 1000);
    expect(formatRelative(target, "en", NOW)).toBe("just now");
  });

  it("returns 'à l'instant' below the one-minute threshold (fr)", () => {
    const target = new Date(NOW.getTime() - 45 * 1000);
    expect(formatRelative(target, "fr", NOW)).toBe("à l'instant");
  });

  it("formats minutes ago in English", () => {
    const target = new Date(NOW.getTime() - 5 * 60 * 1000);
    expect(formatRelative(target, "en", NOW)).toMatch(/5 minutes ago/);
  });

  it("formats minutes ago in French", () => {
    const target = new Date(NOW.getTime() - 5 * 60 * 1000);
    expect(formatRelative(target, "fr", NOW)).toMatch(/il y a 5 minutes/);
  });

  it("formats hours ago", () => {
    const target = new Date(NOW.getTime() - 3 * 60 * 60 * 1000);
    expect(formatRelative(target, "en", NOW)).toMatch(/3 hours ago/);
  });

  it("formats days ago", () => {
    const target = new Date(NOW.getTime() - 2 * 24 * 60 * 60 * 1000);
    expect(formatRelative(target, "en", NOW)).toMatch(/2 days ago/);
  });

  it("accepts an ISO string", () => {
    const iso = new Date(NOW.getTime() - 60 * 60 * 1000).toISOString();
    expect(formatRelative(iso, "en", NOW)).toMatch(/hour ago/);
  });

  it("returns null on a malformed ISO string", () => {
    expect(formatRelative("not-a-date", "en", NOW)).toBeNull();
  });
});

describe("formatAbsolute", () => {
  it("formats a UTC ISO string deterministically", () => {
    expect(formatAbsolute("2026-05-12T03:18:42.515Z")).toBe(
      "2026-05-12 03:18 UTC",
    );
  });

  it("formats a Date instance", () => {
    const date = new Date("2026-01-01T00:00:00Z");
    expect(formatAbsolute(date)).toBe("2026-01-01 00:00 UTC");
  });

  it("normalises offset-bearing timestamps to UTC", () => {
    // 10:00 in +02:00 is 08:00 in UTC.
    expect(formatAbsolute("2026-05-12T10:00:00+02:00")).toBe(
      "2026-05-12 08:00 UTC",
    );
  });

  it("returns null on a malformed input", () => {
    expect(formatAbsolute("nope")).toBeNull();
  });
});