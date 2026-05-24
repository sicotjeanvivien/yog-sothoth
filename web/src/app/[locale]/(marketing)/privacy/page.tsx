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