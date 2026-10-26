import { describe, expect, it } from "vitest";

import type { AnnouncementEntryResponse } from "@/lib/api/schema/zod.gen";
import {
  DISMISSED_IDS_MAX,
  parseDismissedIds,
  pickAnnouncement,
  serializeDismissedIds,
} from "../announcement-state";

function announcement(
  id: number,
  severity: AnnouncementEntryResponse["severity"] = "info",
): AnnouncementEntryResponse {
  return {
    id,
    kind: "release",
    severity,
    message: `announcement ${id}`,
    linkUrl: null,
    startsAt: "2026-07-20T10:00:00Z",
    endsAt: null,
    status: "open",
  };
}

describe("parseDismissedIds", () => {
  it("parses a CSV of ids", () => {
    expect(parseDismissedIds("1,2,3")).toEqual([1, 2, 3]);
  });

  it("returns empty for a missing cookie", () => {
    expect(parseDismissedIds(undefined)).toEqual([]);
    expect(parseDismissedIds("")).toEqual([]);
  });

  it("drops garbage without throwing", () => {
    expect(parseDismissedIds("1,abc,-4,2.5,,3")).toEqual([1, 3]);
  });

  it("deduplicates", () => {
    expect(parseDismissedIds("7,7,7")).toEqual([7]);
  });
});

describe("serializeDismissedIds", () => {
  it("round-trips through parse", () => {
    expect(parseDismissedIds(serializeDismissedIds([1, 2, 3]))).toEqual([
      1, 2, 3,
    ]);
  });

  it("deduplicates and caps, keeping the newest ids", () => {
    const ids = Array.from({ length: DISMISSED_IDS_MAX + 5 }, (_, i) => i + 1);
    const parsed = parseDismissedIds(serializeDismissedIds(ids));
    expect(parsed).toHaveLength(DISMISSED_IDS_MAX);
    // The oldest ids (1..5) are the ones dropped.
    expect(parsed[0]).toBe(6);
    expect(parsed.at(-1)).toBe(DISMISSED_IDS_MAX + 5);
  });
});

describe("pickAnnouncement", () => {
  it("returns the first entry when nothing is dismissed", () => {
    const picked = pickAnnouncement([announcement(1), announcement(2)], []);
    expect(picked?.id).toBe(1);
  });

  it("skips dismissed entries in order", () => {
    const picked = pickAnnouncement([announcement(1), announcement(2)], [1]);
    expect(picked?.id).toBe(2);
  });

  it("returns null when everything is dismissed", () => {
    expect(pickAnnouncement([announcement(1)], [1])).toBeNull();
  });

  it("returns null on an empty active set", () => {
    expect(pickAnnouncement([], [])).toBeNull();
  });

  // Mutation: the API's order kept, or the sort unstable — a later info shown over an earlier
  // warning, or two warnings swapped.
  it("shows the most severe first, the API's order between equals", () => {
    const open = [
      announcement(1, "info"),
      announcement(2, "warning"),
      announcement(3, "critical"),
      announcement(4, "warning"),
    ];
    expect(pickAnnouncement(open, [])?.id).toBe(3);
    expect(pickAnnouncement(open, [3])?.id).toBe(2);
    expect(pickAnnouncement(open, [3, 2])?.id).toBe(4);
  });
});
