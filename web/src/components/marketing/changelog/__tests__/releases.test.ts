/**
 * The hand-edited release data: unique, well-formed versions (each is an anchor id), newest
 * first, no empty content.
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
      // Not strictly: two versions can ship the same day, ordered by version below.
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

  it("marks every line visible or technical", () => {
    for (const release of RELEASES) {
      for (const item of release.sections.flatMap((s) => s.items)) {
        expect(item.text.trim().length, release.version).toBeGreaterThan(0);
        expect(typeof item.technical, `${release.version}: ${item.text}`).toBe("boolean");
      }
    }
  });
});
