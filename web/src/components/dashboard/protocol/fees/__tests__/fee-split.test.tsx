import { renderToStaticMarkup } from "react-dom/server";
import { NextIntlClientProvider } from "next-intl";
import { describe, expect, it, vi } from "vitest";

import messages from "@/messages/en/dashboard.json";

import { FeeSplit } from "../fees-tab";

const fetchSwapsSeries = vi.fn();

vi.mock("@/lib/api/server/protocols", () => ({
  fetchSwapsSeries: (...args: unknown[]) => fetchSwapsSeries(...args),
}));
vi.mock("@/components/dashboard/overview/overview-parts", async (importActual) => ({
  ...(await importActual<object>()),
  BlockHeading: ({ title, children }: { title: string; children?: React.ReactNode }) => (
    <h2>
      {title}
      {children}
    </h2>
  ),
}));
vi.mock("next-intl/server", async () => {
  const { createTranslator } = await import("next-intl");
  const all = (await import("@/messages/en/dashboard.json")).default;
  return {
    getLocale: async () => "en",
    getTranslations: async (namespace: never) => createTranslator({ locale: "en", messages: all, namespace }),
  };
});

const AS_OF = "2026-11-24T08:00:00Z";

const bar = (hoursBefore: number, split: [string | null, string | null]) => ({
  protocol: "meteora_damm_v2",
  from: new Date(Date.parse(AS_OF) - hoursBefore * 3_600_000).toISOString(),
  swaps: 1,
  volumeUsd: "1000",
  feesUsd: "100",
  lpFeesUsd: "80",
  protocolFeesUsd: "15",
  referralFeesUsd: "5",
  poolHours: 1,
  poolHoursValued: 1,
  meteoraDammV2: { claimingFeesUsd: split[0], compoundingFeesUsd: split[1] },
});

const series = (items: unknown[]) => ({
  window: "24h",
  interval: "1h",
  coverageUnit: "poolHours",
  from: new Date(Date.parse(AS_OF) - 24 * 3_600_000).toISOString(),
  asOf: AS_OF,
  coveredSince: "2026-10-13T13:00:00Z",
  open: true,
  opensAt: null,
  late: false,
  items,
  total: [],
});

async function render(items: unknown[]): Promise<string> {
  fetchSwapsSeries.mockResolvedValue(series(items));
  return renderToStaticMarkup(
    <NextIntlClientProvider locale="en" timeZone="UTC" messages={messages}>
      {await FeeSplit({ period: "24h", protocol: "meteora_damm_v2" })}
    </NextIntlClientProvider>,
  );
}

const NOT_MEASURED = messages.Dashboard.Protocol.fees.splitNotMeasured;

describe("FeeSplit", () => {
  // Mutation: the table kept on three rows — the split served and not shown.
  it("lists the LPs' fees claimed and compounded over a period the split covers", async () => {
    const html = await render([bar(2, ["60", "20"]), bar(1, ["70", "10"])]);

    expect(html).toContain("LP — claiming");
    expect(html).toContain("LP — compounding");
    expect(html).toContain("compounding_fee");
    expect(html).not.toContain(NOT_MEASURED);
  });

  // Mutation: the text left out, or the split drawn over a period it does not cover.
  it("keeps the LPs' fees whole and says why over a period the split does not cover", async () => {
    const html = await render([bar(2, [null, null]), bar(1, ["70", "10"])]);

    expect(html).not.toContain("LP — compounding</div>");
    expect(html).toContain("claiming_fee + compounding_fee");
    expect(html).toContain(NOT_MEASURED);
  });
});
