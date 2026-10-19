/** The legal notice (mentions légales), required by the LCEN. */

import { setRequestLocale, getTranslations } from "next-intl/server";
import type { Metadata } from "next";

import { LegalNoticeDocument } from "@/components/marketing/legal-notice/legal-notice-document";
import { pageMetadata } from "@/lib/seo/page-metadata";

type LegalNoticePageProps = {
  params: Promise<{ locale: string }>;
};

export async function generateMetadata({
  params,
}: LegalNoticePageProps): Promise<Metadata> {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: "LegalNotice.meta" });
  return pageMetadata({
    locale,
    path: "/legal-notice",
    title: t("title"),
    description: t("description"),
  });
}

export default async function LegalNoticePage({
  params,
}: LegalNoticePageProps) {
  const { locale } = await params;
  setRequestLocale(locale);

  return (
    <main className="mx-auto max-w-[1280px] px-4 rail:px-8">
      <LegalNoticeDocument />
    </main>
  );
}