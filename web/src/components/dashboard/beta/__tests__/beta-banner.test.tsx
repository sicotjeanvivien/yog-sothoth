import { renderToStaticMarkup } from "react-dom/server";
import { NextIntlClientProvider } from "next-intl";
import { describe, expect, it } from "vitest";

import messages from "@/messages/fr/dashboard.json";

import { BetaBanner } from "../beta-banner";
import { REPORT_ERROR_HREF } from "../beta-state";

function render(): string {
  return renderToStaticMarkup(
    <NextIntlClientProvider locale="fr" timeZone="UTC" messages={messages}>
      <BetaBanner />
    </NextIntlClientProvider>,
  );
}

describe("BetaBanner", () => {
  // Mutation: the link to `CONTACT_EMAIL_HREF` alone — the mail lands without its subject.
  it("links the report to the prefilled mail", () => {
    expect(render()).toContain(`href="${REPORT_ERROR_HREF}"`);
  });

  it("warns, and offers to dismiss", () => {
    const html = render();
    expect(html).toContain("Yog-Scope est en construction.");
    expect(html).toContain("vérifiez-les avant toute décision.");
    expect(html).toContain(">Compris</button>");
  });
});
