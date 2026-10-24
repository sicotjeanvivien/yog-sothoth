import { renderToStaticMarkup } from "react-dom/server";
import { NextIntlClientProvider } from "next-intl";
import { describe, expect, it } from "vitest";

import type { LiveState } from "@/components/dashboard/live-status/live-status";
import messages from "@/messages/fr/dashboard.json";

import { ProductLiveDot } from "../product-live-dot";

function render(state: LiveState): string {
  return renderToStaticMarkup(
    <NextIntlClientProvider locale="fr" timeZone="UTC" messages={messages}>
      <ProductLiveDot state={state} />
    </NextIntlClientProvider>,
  );
}

describe("ProductLiveDot", () => {
  it("is a green dot, named, while the ingestion is live", () => {
    const html = render("live");

    expect(html).toMatch(/bg-dash-up/);
    expect(html).toMatch(/En direct/);
  });

  // Mutation: the dot shown whatever the state — green over a stalled or unreachable ingestion.
  it.each(["delayed", "stale", "offline", "connecting"] as const)("is nothing while %s", (state) => {
    expect(render(state)).toBe("");
  });
});
