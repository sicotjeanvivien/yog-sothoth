import type { ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { createTranslator } from "next-intl";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { ApiClientError } from "@/lib/api/errors";
import type { PageResponseSignalResponse, SignalResponse } from "@/lib/api/schema/zod.gen";
import type { SignalsQuery } from "@/lib/protocol/signals-query";
import messages from "@/messages/en/dashboard.json";

import { SignalsTab } from "../signals-tab";

const fetchProtocolSignals = vi.fn();

vi.mock("@/lib/api/server/signals", () => ({
  SIGNALS_PAGE_SIZE: 50,
  fetchProtocolSignals: (...args: unknown[]) => fetchProtocolSignals(...args),
}));
vi.mock("next-intl/server", async () => {
  const { createTranslator: translator } = await import("next-intl");
  const all = (await import("@/messages/en/dashboard.json")).default;
  return {
    getLocale: async () => "en",
    getTranslations: async (namespace: never) => translator({ locale: "en", messages: all, namespace }),
  };
});
vi.mock("@/i18n/navigation", () => ({
  Link: ({
    href,
    children,
    className,
  }: {
    href: { pathname: string; query: Record<string, string> };
    children: ReactNode;
    className?: string;
  }) => (
    <a href={`${href.pathname}?${new URLSearchParams(href.query)}`} className={className}>
      {children}
    </a>
  ),
}));
vi.mock("@/components/dashboard/overview/overview-states", () => ({
  BlockFailed: ({ kind }: { kind: string }) => <p>failed: {kind}</p>,
}));

const t = createTranslator({ locale: "en", messages, namespace: "Dashboard.Protocol.signals" });

function signal(overrides: Partial<SignalResponse> = {}): SignalResponse {
  const token = { mint: null, name: null, decimals: null, logoUri: null };
  return {
    id: 1,
    detector: "tvl_drain",
    protocol: "meteora_damm_v2",
    poolAddress: "5Q544fKrFoe6tsEbD7S8EmxGTJYAKtTVhAW5Q5pge4j1",
    tokenA: { ...token, symbol: "SOL" },
    tokenB: { ...token, symbol: "USDC" },
    severity: "critical",
    value: "0.124",
    threshold: "0.1",
    valueUnit: "ratio",
    leansToward: null,
    message: null,
    triggeredAt: "2026-11-23T21:14:00Z",
    ...overrides,
  };
}

function page(overrides: Partial<PageResponseSignalResponse> = {}): PageResponseSignalResponse {
  return {
    items: [signal()],
    nextCursor: "older-c",
    prevCursor: null,
    isFirst: true,
    isLast: false,
    ...overrides,
  };
}

const FIRST: SignalsQuery = { severity: null, page: null };

async function render(query: SignalsQuery = FIRST): Promise<string> {
  return renderToStaticMarkup(
    await SignalsTab({
      protocol: "meteora_damm_v2",
      pathname: "/protocols/meteora-damm-v2",
      period: "7d",
      query,
    }),
  );
}

/** Each link's target, by its text. */
function links(html: string): Record<string, string> {
  const found: Record<string, string> = {};
  for (const [, href = "", text = ""] of html.matchAll(/<a href="([^"]*)"[^>]*>([^<]*)<\/a>/g)) {
    found[text] = href.replaceAll("&amp;", "&");
  }
  return found;
}

beforeEach(() => {
  fetchProtocolSignals.mockReset();
});

describe("SignalsTab", () => {
  it("lists each signal: severity, moment, sentence, detector, value and threshold", async () => {
    fetchProtocolSignals.mockResolvedValue(page());
    const html = await render();

    expect(html).toContain("Critical");
    expect(html).toContain("Nov 23, 21:14 UTC");
    expect(html).toContain("TVL drain on ⁨SOL⁩ / ⁨USDC⁩: 12.4%");
    expect(html).toContain("tvl_drain");
    expect(html).toContain("12.4% <span");
    expect(html).toContain("· 10%");
    expect(html).toContain(t("periodIgnored"));
  });

  // Mutation: a filter link keeping the cursor opens the new filter mid-list.
  it("filters from the first page, period and tab kept", async () => {
    fetchProtocolSignals.mockResolvedValue(page({ isFirst: false, prevCursor: "newer-c" }));
    const html = await render({ severity: "warning", page: { cursor: "here", dir: "next" } });
    const to = links(html);

    expect(to["All"]).toBe("/protocols/meteora-damm-v2?tab=signals&period=7d");
    expect(to["Critical"]).toBe("/protocols/meteora-damm-v2?tab=signals&severity=critical&period=7d");
    expect(to["Warning"]).toBeUndefined();
    expect(html).toMatch(/<span aria-current="page"[^>]*>Warning<\/span>/);
  });

  // Mutation: the cursors swapped, or the step back sent without `dir=prev`, which reads older.
  it("steps to the newer and older pages with their cursors, filter kept", async () => {
    fetchProtocolSignals.mockResolvedValue(page({ isFirst: false, prevCursor: "newer-c" }));
    const to = links(await render({ severity: "warning", page: { cursor: "here", dir: "next" } }));

    expect(to[t("newer")]).toBe(
      "/protocols/meteora-damm-v2?tab=signals&severity=warning&cursor=newer-c&dir=prev&period=7d",
    );
    expect(to[t("older")]).toBe(
      "/protocols/meteora-damm-v2?tab=signals&severity=warning&cursor=older-c&period=7d",
    );
  });

  // Mutation: a step past the list's edge, onto an empty page.
  it("greys each step at its edge", async () => {
    fetchProtocolSignals.mockResolvedValue(page({ isLast: true, nextCursor: null }));
    const html = await render();

    expect(links(html)[t("newer")]).toBeUndefined();
    expect(links(html)[t("older")]).toBeUndefined();
    expect(html).toContain(`<span aria-disabled="true"`);
  });

  // Mutation: a refused cursor shown as an error, a tampered link breaks the tab.
  it("reads the first page, filter kept, when the API refuses the cursor", async () => {
    fetchProtocolSignals
      .mockRejectedValueOnce(ApiClientError.http(400, "invalid cursor: malformed payload"))
      .mockResolvedValueOnce(page());
    const html = await render({ severity: "critical", page: { cursor: "bad", dir: "next" } });

    expect(fetchProtocolSignals).toHaveBeenLastCalledWith("meteora_damm_v2", {
      severity: "critical",
      page: null,
    });
    expect(html).toContain("tvl_drain");
    expect(html).not.toContain("failed:");
  });

  // Mutation: every failure retried as the first page hides an API down behind a stale list.
  it("shows the block's error on any other failure", async () => {
    fetchProtocolSignals.mockRejectedValue(ApiClientError.http(500, null));
    expect(await render({ severity: null, page: { cursor: "here", dir: "next" } })).toContain(
      "failed: http",
    );
    expect(fetchProtocolSignals).toHaveBeenCalledTimes(1);

    fetchProtocolSignals.mockReset();
    fetchProtocolSignals.mockRejectedValue(ApiClientError.http(400, null));
    expect(await render()).toContain("failed: http");
    expect(fetchProtocolSignals).toHaveBeenCalledTimes(1);
  });

  // Mutation: one text for both, a filter with no match reads as a quiet protocol.
  it("says whether the list or the filter is empty", async () => {
    fetchProtocolSignals.mockResolvedValue(page({ items: [], nextCursor: null, isLast: true }));
    expect(await render()).toContain(t("empty"));
    expect(await render({ severity: "info", page: null })).toContain(t("emptyFiltered"));
  });
});
