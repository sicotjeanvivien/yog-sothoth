import type { ComponentProps } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { NextIntlClientProvider } from "next-intl";
import { describe, expect, it, vi } from "vitest";

import messages from "@/messages/fr/dashboard.json";

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

// The entries the menu shows without a page, written out rather than
// read from `SIDEBAR_NAV`: turning one into a link must not shrink the
// list this test checks.
const CLOSED_LABELS = [
  "Meteora DAMM v2",
  "Meteora DLMM",
  "Raydium",
  "Orca",
  "Token",
];

function render(): string {
  return renderToStaticMarkup(
    <NextIntlClientProvider locale="fr" timeZone="UTC" messages={messages}>
      <SidebarNavList collapsed={false} onNavigate={() => {}} />
    </NextIntlClientProvider>,
  );
}

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
