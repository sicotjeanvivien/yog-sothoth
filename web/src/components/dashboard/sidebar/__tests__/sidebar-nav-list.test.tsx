import type { ComponentProps } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { NextIntlClientProvider } from "next-intl";
import { describe, expect, it, vi } from "vitest";

import messages from "@/messages/fr/dashboard.json";

import { IngestionContextProvider } from "@/components/dashboard/live-status/live-status-provider";
import type { Half } from "@/components/dashboard/live-status/use-polled";
import type { IngestionResponse } from "@/lib/api/schema/zod.gen";

import { SIDEBAR_NAV, type SidebarNavEntry } from "../sidebar-nav";
import { SidebarNavList } from "../sidebar-nav-list";

// next-intl's navigation needs the Next router; a plain anchor keeps
// what matters here — whether an entry is a link at all.
vi.mock("@/i18n/navigation", async () => {
  const { createElement } = await import("react");
  return {
    Link: (props: ComponentProps<"a">) => createElement("a", props),
    usePathname: () => "/overview",
  };
});

// ⚠️ Written out, not read from `SIDEBAR_NAV`: opening an entry must not shrink this list.
const CLOSED_LABELS = [
  "Meteora DAMM v2",
  "Meteora DLMM",
  "Raydium",
  "Orca",
  "Token",
];

const LIVE: Half<IngestionResponse> = {
  phase: "ready",
  data: { freshness: "live", lastEventAt: null },
};

function render(ingestion: Half<IngestionResponse> = LIVE): string {
  return renderToStaticMarkup(
    <NextIntlClientProvider locale="fr" timeZone="UTC" messages={messages}>
      <IngestionContextProvider value={ingestion}>
        <SidebarNavList collapsed={false} onNavigate={() => {}} />
      </IngestionContextProvider>
    </NextIntlClientProvider>,
  );
}

/** The markup of the `<li>` whose label is `label`. */
function row(html: string, label: string): string {
  const found = html
    .split("<li")
    .find((chunk) => chunk.includes(`<span>${label}</span>`));
  if (found === undefined) throw new Error(`no row for ${label}`);
  return found.slice(0, found.indexOf("</li>"));
}

const DOT = /aria-hidden="true" class="h-2 w-2/;

function flatten(): SidebarNavEntry[] {
  return SIDEBAR_NAV.flatMap((item) =>
    item.kind === "group" ? [...item.entries] : [item],
  );
}

describe("SidebarNavList", () => {
  it("renders Overview as the only link", () => {
    const hrefs = [...render().matchAll(/<a\b[^>]*\shref="([^"]*)"/g)].map(
      (match) => match[1],
    );

    expect(hrefs).toEqual(["/overview"]);
  });

  it.each(CLOSED_LABELS)(
    "renders %s as a disabled span, outside any link",
    (label) => {
      const html = render();
      const closed = new RegExp(
        `<span(?=[^>]*aria-disabled="true")[^>]*><span>${label}</span>`,
      );

      expect(html).toMatch(closed);
      expect(html).not.toMatch(new RegExp(`<a\\b[^>]*>(?:(?!</a>).)*${label}`));
    },
  );

  // Mutations: the lock icon back in `ClosedEntry`, or the "Soon" tag removed.
  it.each(CLOSED_LABELS)("tags %s « Bientôt », with no lock", (label) => {
    const closed = row(render(), label);

    expect(closed).toMatch(/>Bientôt</);
    expect(closed).not.toMatch(/<svg\b/);
  });

  // Mutation: the dot shown on any state, or never.
  it.each([
    ["live", true],
    ["delayed", false],
    ["stale", false],
  ] as const)("shows the DAMM v2 dot when the ingestion is %s: %s", (freshness, shown) => {
    const damm = row(
      render({ phase: "ready", data: { freshness, lastEventAt: null } }),
      "Meteora DAMM v2",
    );

    expect(DOT.test(damm)).toBe(shown);
  });

  it.each([{ phase: "error" }, { phase: "loading" }] as const)(
    "hides the DAMM v2 dot while the ingestion is $phase",
    (ingestion) => {
      expect(DOT.test(row(render(ingestion), "Meteora DAMM v2"))).toBe(false);
    },
  );

  it("gives the dot to DAMM v2 alone", () => {
    const html = render();

    for (const label of CLOSED_LABELS.filter((l) => l !== "Meteora DAMM v2")) {
      expect(DOT.test(row(html, label))).toBe(false);
    }
  });

  it("gives nothing focusable besides the link", () => {
    const html = render();

    expect(html).not.toMatch(/<button\b/);
    expect(html).not.toMatch(/tabindex/i);
  });

  it("opens only Overview in the navigation data", () => {
    const open = flatten().filter((entry) => entry.kind === "open");

    expect(open.map((entry) => entry.key)).toEqual(["overview"]);
  });
});
