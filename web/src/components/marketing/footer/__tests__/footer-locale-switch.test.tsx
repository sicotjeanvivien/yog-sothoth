import type { ComponentProps } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { NextIntlClientProvider } from "next-intl";
import { describe, expect, it, vi } from "vitest";

import { FooterLocaleSwitch } from "../footer-locale-switch";

// next-intl's navigation needs the Next router; this stand-in prefixes the
// locale the way `localePrefix: "always"` does.
vi.mock("@/i18n/navigation", async () => {
  const { createElement } = await import("react");
  return {
    Link: ({ href, locale, ...props }: ComponentProps<"a"> & { locale: string }) =>
      createElement("a", { ...props, href: `/${locale}${href}` }),
    usePathname: () => "/about",
  };
});

function hrefs(): (string | undefined)[] {
  const html = renderToStaticMarkup(
    <NextIntlClientProvider locale="fr" timeZone="UTC" messages={{}}>
      <FooterLocaleSwitch label="Langue" />
    </NextIntlClientProvider>,
  );
  return [...html.matchAll(/<a\b[^>]*\shref="([^"]*)"/g)].map((m) => m[1]);
}

describe("FooterLocaleSwitch", () => {
  // Against a switch that sends every language to the home page.
  it("leads to the same page in each language", () => {
    expect(hrefs()).toEqual(["/en/about", "/fr/about"]);
  });
});
