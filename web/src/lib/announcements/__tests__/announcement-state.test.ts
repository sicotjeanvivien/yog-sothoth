import { describe, expect, it } from "vitest";

import type { AnnouncementResponse } from "@/lib/api/schema/announcement";
import {
  DISMISSED_IDS_MAX,
  parseDismissedIds,
  pickAnnouncement,
  serializeDismissedIds,
} from "../announcement-state";

function announcement(id: number): AnnouncementResponse {
  return {
    id,
    kind: "release",
    severity: "info",
    message: `announcement ${id}`,
    linkUrl: null,
    startsAt: "2026-07-20T10:00:00Z",
    endsAt: null,
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
});
