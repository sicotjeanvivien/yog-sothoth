import type { ComponentProps } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { NextIntlClientProvider } from "next-intl";
import { describe, expect, it, vi } from "vitest";

import messages from "@/messages/fr/dashboard.json";

import { PLATFORMS, SIDEBAR_NAV } from "../sidebar-nav";
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
const CLOSED_LABELS = ["DAMM v2", "DLMM", "Raydium", "Orca", "Token"];

// The closed rows whose icon is drawn; the platforms have a letter, the products nothing.
const DRAWN_ICONS = new Set(["Token"]);

function render(): string {
  return renderToStaticMarkup(
    <NextIntlClientProvider locale="fr" timeZone="UTC" messages={messages}>
      <SidebarNavList collapsed={false} onNavigate={() => {}} />
    </NextIntlClientProvider>,
  );
}

/** The markup of the row labelled `label`, up to the next `<li`. */
function row(html: string, label: string): string {
  const found = html
    .split("<li")
    // React escapes the apostrophe of « Vue d'ensemble ».
    .find((chunk) => chunk.includes(`<span>${label.replace("'", "&#x27;")}</span>`));
  if (found === undefined) throw new Error(`no row for ${label}`);
  return found.split("</li>")[0] ?? "";
}

const ICON = /<span aria-hidden="true"[^>]*>(?:<svg\b|[A-Z]<)/;

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

  // Mutation: a platform with products drawn as a closed entry — « Bientôt » twice over.
  it("captions Meteora, which no product opens yet, with no tag and no link", () => {
    const meteora = row(render(), "Meteora");

    expect(meteora).not.toMatch(/<a\b|aria-disabled|Bientôt/);
  });

  // Mutation: the products rendered beside their platform, outside its list.
  it("lists DAMM v2 and DLMM under Meteora", () => {
    const html = render();
    const id = /<span id="([^"]+)"[^>]*>(?:(?!<\/li>).)*<span>Meteora<\/span>/.exec(html)?.[1];
    const list = new RegExp(`<ul aria-labelledby="${id}"[^>]*>(.*?)</ul>`).exec(html)?.[1] ?? "";

    expect(list).toMatch(/<span>DAMM v2<\/span>(?:(?!<\/li>).)*<\/li><li>.*<span>DLMM<\/span>/);
  });

  // Mutation: the icon rendered on the collapsed rail only.
  it.each(["Vue d'ensemble", "Meteora", "Raydium", "Orca", "Token"])("gives %s its icon", (label) => {
    expect(row(render(), label)).toMatch(ICON);
  });

  // Mutation: a product drawn with its platform's icon — two « M » stacked.
  it.each(["DAMM v2", "DLMM"])("gives the product %s no icon", (label) => {
    expect(row(render(), label)).not.toMatch(ICON);
  });

  it("gives nothing focusable besides the link", () => {
    const html = render();

    expect(html).not.toMatch(/<button\b/);
    expect(html).not.toMatch(/tabindex/i);
  });

  it("opens only Overview in the navigation data", () => {
    const open = [
      ...SIDEBAR_NAV.filter((item) => item.kind === "open"),
      ...PLATFORMS.flatMap((platform) => platform.products).filter((p) => p.kind === "open"),
    ];

    expect(open.map((entry) => entry.key)).toEqual(["overview"]);
  });
});
