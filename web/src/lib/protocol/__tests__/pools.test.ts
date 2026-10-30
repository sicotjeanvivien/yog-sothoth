import { describe, expect, it } from "vitest";

import { birthSlotsOf, birthTotalOf, lifeCountOf, lockKey } from "../pools";

const AS_OF = "2026-10-25T03:00:00Z";

const hour = (h: number) => new Date(Date.parse(AS_OF) - (24 - h) * 3_600_000).toISOString();

describe("birthSlotsOf", () => {
  // Mutation: the period not passed through — the last hour drawn on 24 hourly slots.
  it("draws the last hour on its twelve tranches", () => {
    const at = new Date(Date.parse(AS_OF) - 5 * 60_000).toISOString();
    const slots = birthSlotsOf("1h", AS_OF, [{ from: at, pools: 2 }]);

    expect(slots).toHaveLength(12);
    expect(slots.at(-1)?.value).toBe(2);
  });

  // Mutation: a slot without a bar read as zero — an hour not measured drawn as one without birth.
  it("carries each hour's births, and nothing where no bar is", () => {
    const slots = birthSlotsOf("24h", AS_OF, [
      { from: hour(22), pools: 3 },
      { from: hour(23), pools: 0 },
    ]);

    expect(slots.slice(21).map((s) => s.value)).toEqual([null, 3, 0]);
  });

  it("sums the hours of a 7 days' slice", () => {
    const hours = Array.from({ length: 6 }, (_, h) => ({
      from: new Date(Date.parse("2026-10-24T21:00:00Z") + h * 3_600_000).toISOString(),
      pools: h + 1,
    }));

    expect(birthSlotsOf("7d", AS_OF, hours).at(-1)?.value).toBe(21);
  });
});

describe("birthTotalOf", () => {
  // Mutation: the total started at zero — a period nothing measured read as one without birth.
  it("sums the slots, and is nothing without any bar", () => {
    const slots = birthSlotsOf("24h", AS_OF, [
      { from: hour(22), pools: 3 },
      { from: hour(23), pools: 4 },
    ]);

    expect(birthTotalOf(slots)).toBe(7);
    expect(birthTotalOf(birthSlotsOf("24h", AS_OF, []))).toBeNull();
  });
});

describe("lockKey", () => {
  // Mutation: a lock not measured read as none — a creation valued before the lock was measured
  // said unlocked.
  it("keeps each lock, and a lock not measured apart from none", () => {
    expect(lockKey("permanent")).toBe("permanent");
    expect(lockKey("vesting")).toBe("vesting");
    expect(lockKey("none")).toBe("none");
    expect(lockKey(null)).toBe("unmeasured");
  });
});

describe("lifeCountOf", () => {
  // Mutation: a window without a row read as 0 — a life cycle not measured shown as one without
  // any change.
  it("is the row's count, and nothing while the window holds no row", () => {
    expect(lifeCountOf([{ opened: 7 }], (r) => r.opened)).toBe(7);
    expect(lifeCountOf([{ opened: 0 }], (r) => r.opened)).toBe(0);
    expect(lifeCountOf([] as { opened: number }[], (r) => r.opened)).toBeNull();
  });
});
