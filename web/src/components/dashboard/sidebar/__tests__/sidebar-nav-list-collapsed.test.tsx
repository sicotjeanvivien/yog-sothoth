import type { ComponentProps } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { NextIntlClientProvider } from "next-intl";
import { describe, expect, it, vi } from "vitest";

import messages from "@/messages/fr/dashboard.json";

import type { SidebarNavItem } from "../sidebar-nav";
import { SidebarNavList } from "../sidebar-nav-list";

vi.mock("@/i18n/navigation", async () => {
  const { createElement } = await import("react");
  return {
    Link: (props: ComponentProps<"a">) => createElement("a", props),
    usePathname: () => "/overview",
  };
});

// Today no group holds an open entry; this menu does, as the first
// protocol page will — next to a group that holds only closed ones.
vi.mock("../sidebar-nav", () => ({
  SIDEBAR_NAV: [
    { kind: "open", key: "overview", href: "/overview", labelKey: "overview" },
    {
      kind: "group",
      labelKey: "protocol",
      entries: [
        { kind: "open", key: "overview", href: "/p", labelKey: "meteoraDammV2" },
        { kind: "closed", key: "raydium", labelKey: "raydium" },
      ],
    },
    {
      kind: "group",
      labelKey: "token",
      entries: [{ kind: "closed", key: "orca", labelKey: "orca" }],
    },
  ] satisfies SidebarNavItem[],
}));

type Node = { tag: string; attrs: string; hidden: boolean; hasLink: boolean };

/**
 * Walks the static markup and reports, for every link, closed entry
 * and group list, whether the collapsed rail hides it (an `lg:hidden`
 * on it or on an ancestor).
 */
function walk(html: string) {
  const links: { attrs: string; hidden: boolean }[] = [];
  const closed: boolean[] = [];
  const groups: { hidden: boolean; hasLink: boolean }[] = [];
  const stack: Node[] = [];
  for (const [, slash, tag = "", attrs = "", self] of html.matchAll(
    /<(\/?)([a-z0-9]+)([^>]*?)(\/?)>/g,
  )) {
    if (slash) {
      const node = stack.pop();
      if (node?.tag === "ul" && node.attrs.includes("aria-labelledby")) {
        groups.push({ hidden: node.hidden, hasLink: node.hasLink });
      }
      continue;
    }
    const hidden =
      (stack.at(-1)?.hidden ?? false) || /\blg:hidden\b/.test(attrs);
    if (tag === "a") {
      links.push({ attrs, hidden });
      for (const node of stack) node.hasLink = true;
    }
    if (attrs.includes('aria-disabled="true"')) closed.push(hidden);
    if (!self) stack.push({ tag, attrs, hidden, hasLink: false });
  }
  return { links, closed, groups };
}

function renderCollapsed() {
  return walk(
    renderToStaticMarkup(
      <NextIntlClientProvider locale="fr" timeZone="UTC" messages={messages}>
        <SidebarNavList collapsed={true} onNavigate={() => {}} />
      </NextIntlClientProvider>,
    ),
  );
}

describe("SidebarNavList on the collapsed rail", () => {
  it("keeps every open entry, in a group or not, its icon centred", () => {
    const { links } = renderCollapsed();

    expect(links).toHaveLength(2);
    for (const link of links) {
      expect(link.hidden).toBe(false);
      expect(link.attrs).toMatch(/\blg:px-0\b/);
    }
  });

  it("hides every closed entry", () => {
    const { closed } = renderCollapsed();

    expect(closed).toEqual([true, true]);
  });

  it("shows a group only when it holds an open entry", () => {
    const { groups } = renderCollapsed();

    expect(groups).toEqual([
      { hidden: false, hasLink: true },
      { hidden: true, hasLink: false },
    ]);
  });
});
