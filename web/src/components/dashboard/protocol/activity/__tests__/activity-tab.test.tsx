import type { ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { NextIntlClientProvider } from "next-intl";
import { describe, expect, it, vi } from "vitest";

import type { SignalResponse } from "@/lib/api/schema/zod.gen";
import messages from "@/messages/en/dashboard.json";

import { ActivityTab } from "../activity-tab";

// A signal the server could have loaded: the tab must not show it.
const { fetchSignals } = vi.hoisted(() => {
  const seeded: SignalResponse = {
    id: 1,
    detector: "flow_imbalance",
    protocol: "meteora_damm_v2",
    poolAddress: "5Q544fKrFoe6tsEbD7S8EmxGTJYAKtTVhAW5Q5pge4j1",
    tokenA: { mint: "So11111111111111111111111111111111111111112", symbol: "SOL", name: "Wrapped SOL", decimals: 9, logoUri: null },
    tokenB: { mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v", symbol: "USDC", name: "USD Coin", decimals: 6, logoUri: null },
    severity: "warning",
    value: "0.75",
    threshold: "0.6",
    valueUnit: "signedRatio",
    leansToward: "tokenB",
    message: null,
    triggeredAt: "2026-11-23T10:00:00Z",
  };
  return { fetchSignals: vi.fn(async () => ({ items: [seeded] })) };
});

vi.mock("@/lib/api/server/signals", () => ({ fetchSignals }));
vi.mock("next-intl/server", () => ({
  getTranslations: async () => (key: string) => key,
}));
vi.mock("@/components/dashboard/overview/overview-parts", () => ({
  BlockHeading: ({ title }: { title: string }) => <h2>{title}</h2>,
}));
vi.mock("@/components/dashboard/live/overview-live", () => ({
  OverviewLive: ({ children }: { children: ReactNode }) => <>{children}</>,
}));
vi.mock("@/components/dashboard/activity/activity-block", () => ({ ActivityBlock: () => null }));
vi.mock("@/components/dashboard/live/live-provider", () => ({
  useLive: () => ({ status: "live", subscribe: () => () => {}, onReopen: () => () => {} }),
}));

describe("ActivityTab", () => {
  // Mutation: the tab seeded again with the latest signals (`fetchSignals` called, its list passed as `initial`).
  it("opens on an empty signals block that says so, and loads no signal", async () => {
    const html = renderToStaticMarkup(
      <NextIntlClientProvider locale="en" timeZone="UTC" messages={messages}>
        {await ActivityTab({ protocol: "meteora_damm_v2" })}
      </NextIntlClientProvider>,
    );

    expect(html).toContain("No signal yet — the detectors are watching.");
    expect(html).not.toContain("SOL");
    expect(fetchSignals).not.toHaveBeenCalled();
  });
});
