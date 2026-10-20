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
    usePathname: () => "/damm",
  };
});

// A platform whose second product is open — the current page — next to one that lists none.
vi.mock("../sidebar-nav", () => ({
  SIDEBAR_NAV: [
    { kind: "open", key: "overview", href: "/overview", labelKey: "overview" },
    {
      kind: "platform",
      key: "meteora",
      labelKey: "meteora",
      products: [
        { kind: "closed", key: "meteoraDlmm", labelKey: "dlmm" },
        { kind: "open", key: "meteoraDammV2", href: "/damm", labelKey: "dammV2" },
      ],
    },
    { kind: "platform", key: "orca", labelKey: "orca", products: [] },
    { kind: "closed", key: "token", labelKey: "token" },
  ] satisfies SidebarNavItem[],
}));

type Node = { tag: string; hidden: boolean };

/** Every link and closed entry, by label, with whether a `rail:hidden` on it or an ancestor hides it. */
function walk(html: string) {
  const rows: { label: string; attrs: string; hidden: boolean }[] = [];
  const stack: Node[] = [];
  let open: { attrs: string; hidden: boolean } | null = null;
  for (const [, slash, tag = "", attrs = "", self, text] of html.matchAll(
    /<(\/?)([a-z0-9]+)([^>]*?)(\/?)>([^<]*)/g,
  )) {
    if (slash) {
      stack.pop();
      continue;
    }
    const hidden = (stack.at(-1)?.hidden ?? false) || /\brail:hidden\b/.test(attrs);
    if (tag === "a" || attrs.includes('aria-disabled="true"')) open = { attrs, hidden };
    if (open && tag === "span" && !attrs.includes("aria-hidden") && text?.trim()) {
      rows.push({ label: text.trim().replace("&#x27;", "'"), ...open });
      open = null;
    }
    if (!self) stack.push({ tag, hidden });
  }
  return rows;
}

function collapsedMarkup() {
  return renderToStaticMarkup(
    <NextIntlClientProvider locale="fr" timeZone="UTC" messages={messages}>
      <SidebarNavList collapsed={true} onNavigate={() => {}} />
    </NextIntlClientProvider>,
  );
}

function rows() {
  return Object.fromEntries(walk(collapsedMarkup()).map((r) => [r.label, r]));
}

describe("SidebarNavList on the collapsed rail", () => {
  // Mutations: `rail:hidden` back on a closed entry, or the products left on the rail.
  it("keeps every row as its icon, and hides the platforms' products", () => {
    const shown = Object.entries(rows())
      .filter(([, row]) => !row.hidden)
      .map(([label]) => label);

    expect(shown).toEqual(["Vue d'ensemble", "Meteora", "Orca", "Token"]);
  });

  // Mutation: the platform linked to its first product, open or not.
  it("links a platform to its first open product", () => {
    expect(rows()["Meteora"]?.attrs).toMatch(/href="\/damm"/);
  });

  // Mutation: the current page's platform left unmarked — the folded rail shows no position.
  it("marks the platform of the current page", () => {
    expect(rows()["Meteora"]?.attrs).toMatch(/rail:shadow-\[inset_2px/);
  });

  it("centres every row", () => {
    for (const label of ["Vue d'ensemble", "Meteora", "Orca", "Token"]) {
      expect(rows()[label]?.attrs).toMatch(/\brail:px-0\b/);
    }
  });

  // Mutation: `rail:hidden` instead of `rail:sr-only` on the tag — a screen reader no longer hears « Bientôt ».
  it("keeps « Bientôt » for screen readers", () => {
    const tags = [...collapsedMarkup().matchAll(/<span class="([^"]*)">Bientôt</g)];

    expect(tags).toHaveLength(3);
    for (const [, classes] of tags) expect(classes).toMatch(/\brail:sr-only\b/);
  });
});
