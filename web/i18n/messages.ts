/**
 * Message loader.
 *
 * The translation bundle is split per namespace under
 * `messages/{locale}/<namespace>.json`. Each file owns exactly one
 * top-level key (the namespace) so merging is a simple
 * object spread — no key conflicts to worry about.
 *
 * Explicit imports rather than `require.context` / dynamic glob:
 *
 *   - each path is statically analysable by the bundler;
 *   - missing namespaces fail loudly at build time;
 *   - IDE jump-to-definition works as expected;
 *   - the list of namespaces is one obvious place to maintain.
 *
 * To add a namespace: create both locale files
 * (`messages/en/<name>.json` and `messages/fr/<name>.json`), then add
 * one line to each `BUNDLES` entry below.
 */

import type { AbstractIntlMessages } from "next-intl";

import type { Locale } from "./routing";

// ── EN bundle ─────────────────────────────────────────────────────────

import enBrand from "@/messages/en/brand.json";
import enHome from "@/messages/en/home.json";
import enDashboard from "@/messages/en/dashboard.json";
import enMarketing from "@/messages/en/marketing.json";
import enAbout from "@/messages/en/about.json";
import enPrivacy from "@/messages/en/privacy.json";
import enLegalNotice from "@/messages/en/legal-notice.json";
import enTerms from "@/messages/en/terms.json";
import enSupportUs from "@/messages/en/support-us.json";
import enChangelog from "@/messages/en/changelog.json";
import enCommon from "@/messages/en/common.json";

// ── FR bundle ─────────────────────────────────────────────────────────

import frBrand from "@/messages/fr/brand.json";
import frHome from "@/messages/fr/home.json";
import frDashboard from "@/messages/fr/dashboard.json";
import frMarketing from "@/messages/fr/marketing.json";
import frAbout from "@/messages/fr/about.json";
import frPrivacy from "@/messages/fr/privacy.json";
import frLegalNotice from "@/messages/fr/legal-notice.json";
import frTerms from "@/messages/fr/terms.json";
import frSupportUs from "@/messages/fr/support-us.json";
import frChangelog from "@/messages/fr/changelog.json";
import frCommon from "@/messages/fr/common.json";

// ── Bundle assembly ──────────────────────────────────────────────────
//
// Each per-locale bundle is the merge of its namespace files. Each
// file contributes a single top-level key, so the spread is conflict
// free as long as that convention is respected.

const BUNDLES: Record<Locale, AbstractIntlMessages> = {
  en: {
    ...enBrand,
    ...enHome,
    ...enDashboard,
    ...enMarketing,
    ...enAbout,
    ...enPrivacy,
    ...enLegalNotice,
    ...enTerms,
    ...enSupportUs,
    ...enChangelog,
    ...enCommon,
  },
  fr: {
    ...frBrand,
    ...frHome,
    ...frDashboard,
    ...frMarketing,
    ...frAbout,
    ...frPrivacy,
    ...frLegalNotice,
    ...frTerms,
    ...frSupportUs,
    ...frChangelog,
    ...frCommon,
  },
};

export function loadMessages(locale: Locale): AbstractIntlMessages {
  return BUNDLES[locale];
}
