/**
 * Privacy page.
 *
 * Minimalist, honest privacy policy: one card per question a visitor
 * (and the GDPR) reasonably asks, listed in privacy-prose.tsx.
 * Designed for the project's actual data footprint: server logs,
 * inbound emails, and the Cloudflare proxy in front of both hosts.
 *
 * Pre-deployment review checklist:
 *   - confirm contact email
 *   - confirm hosting provider (Scaleway, Paris)
 *   - confirm no analytics / no tracking are active
 *   - update "last updated" date
 *
 * The copy also states facts that live in the Cloudflare dashboard, not
 * in this repository. Each one, if changed there, makes the page false:
 *   - both hosts (dashboard and API) are proxied, or "every request goes
 *     through Cloudflare" is wrong for the one that is not;
 *   - Bot Fight Mode, rate-limiting rules and Always Online stay off:
 *     they set `__cf_bm` / `_cfuvid` / `cf_ob_info` and `cf_use_ob`,
 *     and the cookie card declares only `cf_clearance` and `cf_chl_*`;
 *   - Challenge Passage stays at 30 minutes, the lifetime the cookie
 *     card gives `cf_clearance`;
 *   - Web Analytics stays off: it injects a third-party beacon;
 *   - Email Address Obfuscation stays off: it rewrites the `mailto:`
 *     links of this page and /legal-notice, and React's hydration
 *     then finds markup it did not render.
 *
 * The cookies the dashboard sets itself are declared too; a new
 * `document.cookie` anywhere in `web/src` belongs on the cookie card.
 */

import { setRequestLocale, getTranslations } from "next-intl/server";
import type { Metadata } from "next";

import { PrivacyHeader } from "@/components/marketing/privacy/privacy-header";
import { PrivacyProse } from "@/components/marketing/privacy/privacy-prose";

type PrivacyPageProps = {
  params: Promise<{ locale: string }>;
};

export async function generateMetadata({
  params,
}: PrivacyPageProps): Promise<Metadata> {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: "Privacy.meta" });
  return {
    title: t("title"),
    description: t("description"),
  };
}

export default async function PrivacyPage({ params }: PrivacyPageProps) {
  const { locale } = await params;
  setRequestLocale(locale);

  return (
    <main>
      <PrivacyHeader />
      <PrivacyProse />
    </main>
  );
}