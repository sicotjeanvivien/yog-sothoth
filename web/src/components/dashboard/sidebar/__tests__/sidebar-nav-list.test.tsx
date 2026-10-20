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

// ⚠️ Written out, not read from `SIDEBAR_NAV`: opening an entry must not shrink this list.
const CLOSED_LABELS = [
  "Meteora DAMM v2",
  "Meteora DLMM",
  "Raydium",
  "Orca",
  "Token",
];

// The closed entries whose icon is drawn; the protocols have an abbreviation.
const DRAWN_ICONS = new Set(["Token"]);

function render(): string {
  return renderToStaticMarkup(
    <NextIntlClientProvider locale="fr" timeZone="UTC" messages={messages}>
      <SidebarNavList collapsed={false} onNavigate={() => {}} />
    </NextIntlClientProvider>,
  );
}

/** The markup of the `<li>` whose label is `label`. */
function row(html: string, label: string): string {
  const found = html
    .split("<li")
    // React escapes the apostrophe of « Vue d'ensemble ».
    .find((chunk) => chunk.includes(`<span>${label.replace("'", "&#x27;")}</span>`));
  if (found === undefined) throw new Error(`no row for ${label}`);
  return found.slice(0, found.indexOf("</li>"));
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
      const closed = row(render(), label);

      expect(closed).toMatch(/<span(?=[^>]*role="link")(?=[^>]*aria-disabled="true")/);
      expect(closed).not.toMatch(/<a\b/);
    },
  );

  // Mutations: the lock icon back in `ClosedEntry`, or the "Soon" tag removed.
  it.each(CLOSED_LABELS)("tags %s « Bientôt », with no lock", (label) => {
    const closed = row(render(), label);

    expect(closed).toMatch(/>Bientôt</);
    expect(closed.match(/<svg\b/g)?.length ?? 0).toBe(DRAWN_ICONS.has(label) ? 1 : 0);
  });

  // Mutation: the icon rendered on the collapsed rail only.
  it.each(["Vue d'ensemble", ...CLOSED_LABELS])("gives %s its icon", (label) => {
    expect(row(render(), label)).toMatch(
      /<span aria-hidden="true"[^>]*>(?:<svg\b|D2<|DL<|RY<|OR<)/,
    );
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
