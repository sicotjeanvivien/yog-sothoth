import { afterEach, describe, expect, it, vi } from "vitest";

import { CONTACT_EMAIL_HREF } from "@/components/marketing/contact";

import {
  REPORT_ERROR_HREF,
  isBetaDismissed,
  rememberBetaDismissed,
} from "../beta-state";

describe("REPORT_ERROR_HREF", () => {
  // Mutations: another address, another subject, the subject left unencoded.
  it("mails the contact address, subject prefilled and encoded", () => {
    expect(REPORT_ERROR_HREF).toBe(
      `${CONTACT_EMAIL_HREF}?subject=Yog-Scope%20%E2%80%94%20erreur%20de%20donn%C3%A9es`,
    );
  });
});

describe("rememberBetaDismissed", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  // Mutation: a 90-day max-age — the banner stays away three times too long.
  it("writes a 30-day cookie the layout reads as dismissed", () => {
    const doc = { cookie: "" };
    vi.stubGlobal("document", doc);

    rememberBetaDismissed();

    expect(doc.cookie).toBe(
      "yog_beta_dismissed=1; path=/; max-age=2592000; samesite=lax",
    );
    const value = /^yog_beta_dismissed=([^;]*)/.exec(doc.cookie)?.[1];
    expect(isBetaDismissed(value)).toBe(true);
  });
});

describe("isBetaDismissed", () => {
  // Mutation: `raw !== undefined` — any cookie value hides the banner.
  it.each([undefined, "0", "oui"])("shows the banner for %s", (raw) => {
    expect(isBetaDismissed(raw)).toBe(false);
  });
});
