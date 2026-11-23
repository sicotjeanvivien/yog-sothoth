import type { ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { NextIntlClientProvider } from "next-intl";
import { describe, expect, it, vi } from "vitest";

import messages from "@/messages/en/dashboard.json";

import { ActivityTab } from "../activity-tab";

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
  // Mutation: a starting list given back to `useLiveSignals`, the tab seeded again before the stream.
  it("opens on an empty signals block that says so", async () => {
    const html = renderToStaticMarkup(
      <NextIntlClientProvider locale="en" timeZone="UTC" messages={messages}>
        {await ActivityTab({ protocol: "meteora_damm_v2" })}
      </NextIntlClientProvider>,
    );

    expect(html).toContain("No signal yet — the detectors are watching.");
    expect(html).not.toContain('role="table"');
  });
});
