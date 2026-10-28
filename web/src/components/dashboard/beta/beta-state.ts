/**
 * The beta banner's "Got it" cookie. ⚠️ No "use client": the server layout reads it for the
 * first paint, and the client banner writes it.
 */

import { CONTACT_EMAIL_HREF } from "@/components/marketing/contact";
import { writeCookie } from "@/components/shared/write-cookie";

export const BETA_DISMISSED_COOKIE = "yog_beta_dismissed";

export const BETA_COOKIE_MAX_AGE_S = 60 * 60 * 24 * 30;

/** One subject in both locales: it is what sorts the inbox. */
export const REPORT_ERROR_HREF = `${CONTACT_EMAIL_HREF}?subject=${encodeURIComponent(
  "Yog-Scope — erreur de données",
)}`;

export function isBetaDismissed(raw: string | undefined): boolean {
  return raw === "1";
}

export function rememberBetaDismissed() {
  writeCookie(BETA_DISMISSED_COOKIE, true, BETA_COOKIE_MAX_AGE_S);
}
