/**
 * Legal notice page (Mentions légales).
 *
 * Mandatory identification record under French law (LCEN 2004,
 * article 6-III). Lists the editor's corporate identity, the
 * publishing director, the contact address, the hosting provider,
 * and a short IP statement.
 *
 * Pre-deployment review checklist:
 *   - confirm AWSD legal name, SIREN, VAT, share capital,
 *     registered office
 *   - confirm publishing director's full name
 *   - confirm contact email
 *   - confirm hosting provider (Scaleway, Paris) — address and phone
 *   - update "last updated" date
 */

import { setRequestLocale, getTranslations } from "next-intl/server";
import type { Metadata } from "next";

import { LegalNoticeHeader } from "@/components/marketing/legal-notice/legal-notice-header";
import { LegalNoticeContent } from "@/components/marketing/legal-notice/legal-notice-content";

type LegalNoticePageProps = {
  params: Promise<{ locale: string }>;
};

export async function generateMetadata({
  params,
}: LegalNoticePageProps): Promise<Metadata> {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: "LegalNotice.meta" });
  return {
    title: t("title"),
    description: t("description"),
  };
}

export default async function LegalNoticePage({
  params,
}: LegalNoticePageProps) {
  const { locale } = await params;
  setRequestLocale(locale);

  return (
    <main>
      <LegalNoticeHeader />
      <LegalNoticeContent />
    </main>
  );
}