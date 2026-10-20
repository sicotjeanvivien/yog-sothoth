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

// A group holding an open entry, next to one holding only closed ones.
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

/** For every link, closed entry, group and caption, whether a `rail:hidden` on it or an ancestor hides it. */
function walk(html: string) {
  const links: { attrs: string; hidden: boolean }[] = [];
  const closed: boolean[] = [];
  const groups: { hidden: boolean; hasLink: boolean }[] = [];
  const captions: boolean[] = [];
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
      (stack.at(-1)?.hidden ?? false) || /\brail:hidden\b/.test(attrs);
    if (tag === "a") {
      links.push({ attrs, hidden });
      for (const node of stack) node.hasLink = true;
    }
    if (attrs.includes('aria-disabled="true"')) closed.push(hidden);
    if (tag === "span" && / id="/.test(attrs)) captions.push(hidden);
    if (!self) stack.push({ tag, attrs, hidden, hasLink: false });
  }
  return { links, closed, groups, captions };
}

function collapsedMarkup() {
  return renderToStaticMarkup(
    <NextIntlClientProvider locale="fr" timeZone="UTC" messages={messages}>
      <SidebarNavList collapsed={true} onNavigate={() => {}} />
    </NextIntlClientProvider>,
  );
}

function renderCollapsed() {
  return walk(collapsedMarkup());
}

describe("SidebarNavList on the collapsed rail", () => {
  it("keeps every open entry, in a group or not, its icon centred", () => {
    const { links } = renderCollapsed();

    expect(links).toHaveLength(2);
    for (const link of links) {
      expect(link.hidden).toBe(false);
      expect(link.attrs).toMatch(/\brail:px-0\b/);
    }
  });

  // Mutation: `rail:hidden` back on a closed entry — the rail shows one entry out of six.
  it("keeps every closed entry", () => {
    const { closed } = renderCollapsed();

    expect(closed).toEqual([false, false]);
  });

  // Mutation: `rail:hidden` instead of `rail:sr-only` on the tag — a screen reader no longer hears « Bientôt ».
  it("keeps « Bientôt » for screen readers", () => {
    const tags = [...collapsedMarkup().matchAll(/<span class="([^"]*)">Bientôt</g)];

    expect(tags).toHaveLength(2);
    for (const [, classes] of tags) expect(classes).toMatch(/\brail:sr-only\b/);
  });

  // Mutation: a group hidden when it holds no open entry, as when only open ones had an icon.
  it("keeps every group, its caption hidden", () => {
    const { groups, captions } = renderCollapsed();

    expect(groups.map((group) => group.hidden)).toEqual([false, false]);
    expect(captions).toEqual([true, true]);
  });
});
