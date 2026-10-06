/**
 * Guard rails for the hand-edited release data. The operator appends a
 * block at each release — these tests catch the realistic slips:
 * duplicated or malformed version (it doubles as the anchor id the
 * announcements' link_url targets), wrong ordering, empty content.
 */

import { describe, expect, it } from "vitest";

import { RELEASES } from "../releases";

describe("RELEASES", () => {
  it("is not empty", () => {
    expect(RELEASES.length).toBeGreaterThan(0);
  });

  it("has unique vX.Y.Z versions (they double as anchor ids)", () => {
    const versions = RELEASES.map((r) => r.version);
    expect(new Set(versions).size).toBe(versions.length);
    for (const version of versions) {
      expect(version).toMatch(/^v\d+\.\d+\.\d+$/);
    }
  });

  it("has valid ISO dates, newest first", () => {
    const times = RELEASES.map((r) => {
      expect(r.date).toMatch(/^\d{4}-\d{2}-\d{2}$/);
      const t = new Date(r.date).getTime();
      expect(Number.isNaN(t)).toBe(false);
      return t;
    });
    times.slice(1).forEach((time, i) => {
      // `times[i]` is the previous entry; the slice guarantees it exists.
      // Not strictly: two versions can ship the same day — an urgent fix
      // goes out at once — so the order within a day is the version's,
      // checked below.
      expect(times[i]!).toBeGreaterThanOrEqual(time);
    });
  });

  it("is ordered by version, newest first", () => {
    const parts = RELEASES.map((r) =>
      r.version.slice(1).split(".").map(Number),
    );
    parts.slice(1).forEach((next, i) => {
      // `parts[i]` is the previous entry; the slice guarantees it exists.
      const previous = parts[i]!;
      const newer =
        previous[0]! - next[0]! ||
        previous[1]! - next[1]! ||
        previous[2]! - next[2]!;
      expect(
        newer,
        `${RELEASES[i]!.version} before ${RELEASES[i + 1]!.version}`,
      ).toBeGreaterThan(0);
    });
  });

  it("every release has a summary and at least one non-empty section", () => {
    for (const release of RELEASES) {
      expect(release.summary.trim().length).toBeGreaterThan(0);
      expect(release.sections.length).toBeGreaterThan(0);
      for (const section of release.sections) {
        expect(section.items.length).toBeGreaterThan(0);
      }
    }
  });
});
