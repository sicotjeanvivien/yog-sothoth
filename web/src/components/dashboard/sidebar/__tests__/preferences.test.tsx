import { renderToStaticMarkup } from "react-dom/server";
import { NextIntlClientProvider } from "next-intl";
import { describe, expect, it, vi } from "vitest";

import messages from "@/messages/fr/dashboard.json";

import { Preferences } from "../preferences";

vi.mock("next/navigation", () => ({ useSearchParams: () => new URLSearchParams() }));
vi.mock("@/i18n/navigation", () => ({ usePathname: () => "/overview" }));
vi.mock("@/components/shared/locale-switch", () => ({ LocaleSwitch: () => null }));

function expanded(open: boolean, folded: boolean): string | undefined {
  const html = renderToStaticMarkup(
    <NextIntlClientProvider locale="fr" timeZone="UTC" messages={messages}>
      <Preferences open={open} onToggle={() => {}} collapsed={folded} folded={folded} />
    </NextIntlClientProvider>,
  );
  return /aria-expanded="(true|false)"/.exec(html)?.[1];
}

describe("Preferences", () => {
  // Mutation: `aria-expanded={open}` — a screen reader hears an open panel the folded rail hides.
  it("read as closed on the folded rail, even when open", () => {
    expect(expanded(true, true)).toBe("false");
  });

  it("read as open when the panel shows", () => {
    expect(expanded(true, false)).toBe("true");
  });
});
