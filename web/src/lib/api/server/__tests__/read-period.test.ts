import { beforeEach, describe, expect, it, vi } from "vitest";

import { ApiClientError } from "@/lib/api/errors";
import { OVERVIEW_PERIODS } from "@/lib/overview/period";

import { fetchWindows } from "../protocols";
import { readPeriod } from "../read-period";

vi.mock("../protocols", () => ({ fetchWindows: vi.fn() }));

const answer = vi.mocked(fetchWindows);

const row = (window: string, open: boolean, opensAt: string | null) => ({
  window,
  open,
  opensAt,
  from: null,
  asOf: null,
  coveredSince: null,
  late: false,
});

beforeEach(() => {
  answer.mockReset();
  answer.mockResolvedValue({
    items: [row("7d", false, "2026-11-02T09:00:00Z"), row("30d", false, null), row("1h", true, null)],
  } as never);
});

describe("readPeriod", () => {
  // Mutation: the guard dropped — `?period=30d` runs the reads the API holds closed.
  it("falls back to 24h on a period held closed", async () => {
    await expect(readPeriod("30d", OVERVIEW_PERIODS)).resolves.toBe("24h");
  });

  // Mutation: every closed period sent to 24h — a period opening soon never says when.
  it("keeps an open period, and one closed until a date", async () => {
    await expect(readPeriod("1h", OVERVIEW_PERIODS)).resolves.toBe("1h");
    await expect(readPeriod("7d", OVERVIEW_PERIODS)).resolves.toBe("7d");
  });

  // Mutation: the failure left to bubble — a page lost for the sake of its period.
  it("falls back to 24h when the windows cannot be read", async () => {
    answer.mockRejectedValue(ApiClientError.validation(["down"]));

    await expect(readPeriod("7d", OVERVIEW_PERIODS)).resolves.toBe("24h");
  });

  // Mutation: the catch widened — a bug in the page read as an API failure.
  it("lets anything but an API failure through", async () => {
    answer.mockRejectedValue(new TypeError("bug"));

    await expect(readPeriod("7d", OVERVIEW_PERIODS)).rejects.toThrow("bug");
  });

  // Mutation: the read made first — every default page held up on it.
  it("reads nothing for the default period", async () => {
    await expect(readPeriod(undefined, OVERVIEW_PERIODS)).resolves.toBe("24h");
    expect(answer).not.toHaveBeenCalled();
  });
});
