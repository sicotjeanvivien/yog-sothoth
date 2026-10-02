/**
 * Privacy page.
 *
 * Minimalist, honest privacy policy. Six cards answering the
 * questions a visitor (and the GDPR) reasonably asks. Designed for
 * the project's actual data footprint: server logs and inbound
 * emails, nothing else.
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
 *   - Bot Fight Mode and rate-limiting rules stay off: they set
 *     `__cf_bm` / `_cfuvid` on ordinary visits, and the cookie card
 *     declares `cf_clearance` alone;
 *   - Challenge Passage stays at 30 minutes, the lifetime the cookie
 *     card gives `cf_clearance`;
 *   - Web Analytics stays off: it injects a third-party beacon.
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