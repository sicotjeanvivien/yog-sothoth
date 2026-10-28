import { renderToStaticMarkup } from "react-dom/server";
import { NextIntlClientProvider } from "next-intl";
import { describe, expect, it, vi } from "vitest";

import brand from "@/messages/fr/brand.json";
import dashboard from "@/messages/fr/dashboard.json";

import { Brand } from "../sidebar";

vi.mock("next/navigation", () => ({ useSearchParams: () => new URLSearchParams() }));
vi.mock("@/i18n/navigation", () => ({
  usePathname: () => "/overview",
  Link: ({ href, children, ...props }: { href: string; children: React.ReactNode }) => (
    <a href={href} {...props}>
      {children}
    </a>
  ),
}));
vi.mock("@/components/shared/logo-mark", () => ({ LogoMark: () => null }));

function render(props: { collapsed?: boolean; small?: boolean }): string {
  return renderToStaticMarkup(
    <NextIntlClientProvider locale="fr" timeZone="UTC" messages={{ ...brand, ...dashboard }}>
      <Brand {...props} />
    </NextIntlClientProvider>,
  );
}

const WORDMARK = /<span class="([^"]*)"><span[^>]*>Yog-Scope<\/span><span[^>]*>Bêta<\/span><\/span>/;

describe("Brand", () => {
  // Mutations: `rail:hidden` on the badge — the folded rail silences "Bêta" for a screen
  // reader; `rail:sr-only` on the name alone — the empty column shifts the logo off-centre.
  it("folds the name and its badge together, still read aloud", () => {
    const html = render({ collapsed: true });
    expect(WORDMARK.exec(html)?.[1]).toContain("rail:sr-only");
    expect(html).not.toContain("rail:hidden");
  });

  it("shows the badge under the name, on the rail and in the mobile bar", () => {
    expect(WORDMARK.exec(render({}))?.[1]).not.toContain("rail:sr-only");
    expect(render({ small: true })).toMatch(WORDMARK);
  });
});
