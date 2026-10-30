/**
 * Tests for the generated wire schemas: what each refuses is what the API's
 * contract refuses. Regenerate with `npm run gen:api` after a DTO change.
 */

import { describe, expect, it } from "vitest";
import {
  zAnnouncementEntryResponse,
  zPageResponseSignalResponse,
  zProblemDetailsResponse,
  zProtocolSwapActivityResponse,
  zSignalResponse,
} from "../schema/zod.gen";


function validAnnouncement() {
  return {
    id: 1,
    kind: "release",
    severity: "info",
    message: "v0.1.0 is live — signal engine and alerts.",
    linkUrl: "/changelog#v0.1.0",
    startsAt: "2026-10-06T10:00:00.000000Z",
    endsAt: null,
    status: "open",
  };
}

describe("zAnnouncementEntryResponse", () => {
  it("accepts a complete valid announcement", () => {
    const parsed = zAnnouncementEntryResponse.parse(validAnnouncement());
    expect(parsed.id).toBe(1);
    expect(parsed.kind).toBe("release");
  });

  it("accepts a null linkUrl and a relative one", () => {
    expect(
      zAnnouncementEntryResponse.parse({ ...validAnnouncement(), linkUrl: null })
        .linkUrl,
    ).toBeNull();
    // Mutation: a link required to be absolute — in-app paths are the main use.
    expect(
      zAnnouncementEntryResponse.parse({ ...validAnnouncement(), linkUrl: "/changelog" })
        .linkUrl,
    ).toBe("/changelog");
  });

  it("rejects an unknown kind or severity", () => {
    expect(() =>
      zAnnouncementEntryResponse.parse({ ...validAnnouncement(), kind: "party" }),
    ).toThrow();
    expect(() =>
      zAnnouncementEntryResponse.parse({ ...validAnnouncement(), severity: "loud" }),
    ).toThrow();
  });
});

describe("zProblemDetailsResponse", () => {
  function validProblem() {
    return {
      type: "about:blank",
      title: "Bad Request",
      status: 400,
      detail: "limit must be between 1 and 200",
    };
  }

  it("accepts a well-formed RFC 9457 problem", () => {
    const parsed = zProblemDetailsResponse.parse(validProblem());
    expect(parsed.type).toBe("about:blank");
    expect(parsed.title).toBe("Bad Request");
    expect(parsed.status).toBe(400);
    expect(parsed.detail).toBe("limit must be between 1 and 200");
  });

  it("accepts a future type URI", () => {
    const parsed = zProblemDetailsResponse.parse({
      ...validProblem(),
      type: "https://api.yog-sothoth.fr/errors/invalid-cursor",
    });
    expect(parsed.type).toBe("https://api.yog-sothoth.fr/errors/invalid-cursor");
  });

  it("rejects a body missing required RFC 9457 fields", () => {
    expect(() => zProblemDetailsResponse.parse({ error: "old format" })).toThrow();
    expect(() => zProblemDetailsResponse.parse({ detail: "no other fields" })).toThrow();
  });

  it("rejects a non-numeric status", () => {
    expect(() =>
      zProblemDetailsResponse.parse({ ...validProblem(), status: "400" }),
    ).toThrow();
  });
});

describe("zProtocolSwapActivityResponse", () => {
  const validTotals = () => ({
    protocol: "meteora_damm_v2",
    volumeUsd: "508193.05",
    feesUsd: "391.03",
    lpFeesUsd: "312.82",
    protocolFeesUsd: "78.21",
    referralFeesUsd: "0",
    swaps: 1246,
    pools: 359,
    poolHours: 1246,
    poolHoursValued: 767,
  });

  it("accepts a complete valid payload", () => {
    const parsed = zProtocolSwapActivityResponse.parse(validTotals());
    expect(parsed.volumeUsd).toBe("508193.05");
    expect(parsed.poolHoursValued).toBe(767);
  });

  it("accepts the null distinct pools of a previous window not yet written", () => {
    const parsed = zProtocolSwapActivityResponse.parse({ ...validTotals(), pools: null });
    expect(parsed.pools).toBeNull();
  });

  it("rejects a USD amount sent as a JS number (precision contract)", () => {
    expect(() =>
      zProtocolSwapActivityResponse.parse({ ...validTotals(), volumeUsd: 508193.05 }),
    ).toThrow();
  });

  it("rejects a negative or fractional count", () => {
    expect(() => zProtocolSwapActivityResponse.parse({ ...validTotals(), swaps: -1 })).toThrow();
    expect(() =>
      zProtocolSwapActivityResponse.parse({ ...validTotals(), poolHours: 1.5 }),
    ).toThrow();
  });
});

// The shape of both the list items and the SSE events.
function validSignal() {
  return {
    id: 739,
    detector: "price_oracle_deviation",
    protocol: "meteora_damm_v2",
    poolAddress: "5NMi3SSebB7MyP17Sf5SwXh6nTnPPp1Ctb1G45NnRKuZ",
    tokenA: {
      mint: "So11111111111111111111111111111111111111112",
      symbol: "SOL",
      name: "Wrapped SOL",
      decimals: 9,
      logoUri: null,
    },
    tokenB: {
      mint: null,
      symbol: null,
      name: null,
      decimals: 0,
      logoUri: null,
    },
    severity: "critical",
    value: "1.1059558665865029280736523078",
    threshold: "0.0500",
    valueUnit: "signedRatio",
    flowsToward: null,
    message: "spot price deviates 1.1060 from oracle (spot 0.000000582488, oracle 0.000000276591)",
    triggeredAt: "2026-07-02T10:03:31.056847Z",
  };
}

describe("zSignalResponse", () => {
  it("accepts a real payload", () => {
    expect(() => zSignalResponse.parse(validSignal())).not.toThrow();
  });

  it("accepts a negative value (a deviation can be below the oracle)", () => {
    const parsed = zSignalResponse.parse({ ...validSignal(), value: "-0.2157" });
    expect(parsed.value).toBe("-0.2157");
  });

  it("accepts null threshold and message", () => {
    const parsed = zSignalResponse.parse({
      ...validSignal(),
      threshold: null,
      message: null,
    });
    expect(parsed.threshold).toBeNull();
    expect(parsed.message).toBeNull();
  });

  it("rejects an unknown severity (closed set)", () => {
    expect(() =>
      zSignalResponse.parse({ ...validSignal(), severity: "panic" }),
    ).toThrow();
  });

  it("rejects a value sent as a JS number (precision contract)", () => {
    expect(() =>
      zSignalResponse.parse({ ...validSignal(), value: 1.1059 }),
    ).toThrow();
  });

  it("rejects a fractional id", () => {
    expect(() => zSignalResponse.parse({ ...validSignal(), id: 1.5 })).toThrow();
  });
});

/** A first page of signals with more after; overrides give the other shapes. */
function validSignalsPage(overrides: Record<string, unknown> = {}) {
  return {
    items: [validSignal()],
    nextCursor: "next-cursor-opaque",
    prevCursor: null,
    isFirst: true,
    isLast: false,
    ...overrides,
  };
}

describe("zPageResponseSignalResponse", () => {
  it("accepts a first page with more data after", () => {
    const parsed = zPageResponseSignalResponse.parse(validSignalsPage());
    expect(parsed.items).toHaveLength(1);
    expect(parsed.prevCursor).toBeNull();
    expect(parsed.isFirst).toBe(true);
    expect(parsed.isLast).toBe(false);
  });

  it("accepts a single-page result (no neighbours)", () => {
    const parsed = zPageResponseSignalResponse.parse(
      validSignalsPage({ items: [], nextCursor: null, isLast: true }),
    );
    expect(parsed.items).toHaveLength(0);
    expect(parsed.nextCursor).toBeNull();
  });

  it("rejects items that fail individual validation", () => {
    expect(() =>
      zPageResponseSignalResponse.parse(
        validSignalsPage({ items: [{ ...validSignal(), poolAddress: "" }] }),
      ),
    ).toThrow();
  });

  it("rejects a missing cursor or boundary flag", () => {
    for (const key of ["nextCursor", "prevCursor", "isFirst", "isLast"]) {
      const page: Record<string, unknown> = validSignalsPage();
      delete page[key];
      expect(zPageResponseSignalResponse.safeParse(page).success, key).toBe(false);
    }
  });

  it("rejects a non-boolean isFirst flag", () => {
    expect(() =>
      zPageResponseSignalResponse.parse(validSignalsPage({ isFirst: "true" })),
    ).toThrow();
  });
});

// One test per wire form the contract has to state on its own: each is red when
// the rule that states it leaves the spec, then the zod is regenerated.
describe("the contract's wire forms", () => {
  it("refuses an amount out of format (the `UnsignedDecimal` pattern)", () => {
    for (const threshold of ["abc", "-1", "1e-7", "", " 25"]) {
      expect(zSignalResponse.safeParse({ ...validSignal(), threshold }).success).toBe(false);
    }
  });

  it("refuses a date out of RFC 3339, and keeps the offsets `to_rfc3339` writes", () => {
    for (const triggeredAt of ["2026-05-21T10:01:35", "2026-05-21", "yesterday"]) {
      expect(zSignalResponse.safeParse({ ...validSignal(), triggeredAt }).success).toBe(false);
    }
    const offset = { ...validSignal(), triggeredAt: "2026-05-21T10:01:35+00:00" };
    expect(zSignalResponse.safeParse(offset).success).toBe(true);
  });

  it("refuses a missing key where the API writes null (the required walk)", () => {
    const { threshold, ...rest } = validSignal();
    void threshold;
    expect(zSignalResponse.safeParse(rest).success).toBe(false);
  });

  it("reads a count as a number, never a BigInt (no int64 in the spec)", () => {
    const parsed = zSignalResponse.parse(validSignal());
    expect(typeof parsed.id).toBe("number");
    const asString = { ...validSignal(), id: "739" };
    expect(zSignalResponse.safeParse(asString).success).toBe(false);
  });
});
