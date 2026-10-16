/** The legal notice (mentions légales), required by the LCEN, article 6-III. */

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