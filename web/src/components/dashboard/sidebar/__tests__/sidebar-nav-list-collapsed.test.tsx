import type { ComponentProps } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { NextIntlClientProvider } from "next-intl";
import { describe, expect, it, vi } from "vitest";

import messages from "@/messages/fr/dashboard.json";

import type { SidebarNavItem } from "../sidebar-nav";
import { SidebarNavList } from "../sidebar-nav-list";

const nav = vi.hoisted(() => ({ pathname: "/damm" }));

vi.mock("@/i18n/navigation", async () => {
  const { createElement } = await import("react");
  return {
    Link: (props: ComponentProps<"a">) => createElement("a", props),
    usePathname: () => nav.pathname,
  };
});

// A platform whose second product is open — the current page —, one whose products are all
// closed, and one that lists none.
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
    {
      kind: "platform",
      key: "raydium",
      labelKey: "raydium",
      products: [{ kind: "closed", key: "meteoraDlmm", labelKey: "dlmm" }],
    },
    { kind: "platform", key: "orca", labelKey: "orca", products: [] },
    { kind: "closed", key: "token", labelKey: "token" },
  ] satisfies SidebarNavItem[],
}));

type Node = { tag: string; hidden: boolean };

/** Every link, closed entry and caption, by label, with whether a `rail:hidden` on it or an ancestor hides it. */
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
    if (tag === "a" || attrs.includes('aria-disabled="true"') || (tag === "span" && / id="/.test(attrs))) {
      open = { attrs, hidden };
    }
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

    expect(shown).toEqual(["Vue d'ensemble", "Meteora", "Raydium", "Orca", "Token"]);
  });

  // Mutation: the platform linked to its first product, open or not.
  it("links a platform to its first open product", () => {
    expect(rows()["Meteora"]?.attrs).toMatch(/href="\/damm"/);
  });

  // Mutations: the current page's platform left unmarked, or marked on its products' own pages only.
  it.each(["/damm", "/damm/pool"])("marks the platform of the current page, %s", (pathname) => {
    nav.pathname = pathname;
    expect(rows()["Meteora"]?.attrs).toMatch(/rail:shadow-\[inset_2px/);
    nav.pathname = "/damm";
  });

  // Mutation: the caption left as plain text — folded, nothing says its products are not open yet.
  it("tells that a platform with no open product is not ready yet", () => {
    const caption = rows()["Raydium"]?.attrs ?? "";

    expect(caption).toMatch(/title="Raydium — bientôt"/);
    expect(caption).toMatch(/rail:text-dash-ink-3/);
    expect(collapsedMarkup()).toMatch(/<span aria-hidden="true" class="[^"]*\brail:opacity-50\b[^"]*"><img[^>]*logo_raydium/);
  });

  // Mutation: `rail:px-0` dropped from a platform's link or caption — its icon sits off-centre.
  it("centres every row", () => {
    for (const label of ["Vue d'ensemble", "Meteora", "Raydium", "Orca", "Token"]) {
      expect(rows()[label]?.attrs).toMatch(/\brail:px-0\b/);
    }
  });

  // Mutation: the logo sized by its attributes alone — Tailwind's `height: auto` gives it the file's proportions.
  it("gives each platform's logo the size of its box", () => {
    const logos = [...collapsedMarkup().matchAll(/<img[^>]*logo_[a-z]+\.svg[^>]*>/g)].map(([tag]) => tag);

    expect(logos).toHaveLength(3);
    for (const tag of logos) expect(tag).toMatch(/class="(?:[^"]* )?size-5[ "]/);
  });

  // Mutation: `rail:hidden` instead of `rail:sr-only` on the tag — a screen reader no longer hears « Bientôt ».
  it("keeps « Bientôt » for screen readers", () => {
    const tags = [...collapsedMarkup().matchAll(/<span class="([^"]*)">Bientôt</g)];

    expect(tags).toHaveLength(5);
    for (const [, classes] of tags) expect(classes).toMatch(/\brail:sr-only\b/);
  });
});
