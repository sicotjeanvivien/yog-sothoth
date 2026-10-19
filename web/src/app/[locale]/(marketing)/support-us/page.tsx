/** The support page: three free ways to help, then a donation in SOL. */

import { setRequestLocale, getTranslations } from "next-intl/server";
import type { Metadata } from "next";

import { SitePageHeader } from "@/components/marketing/site-page-header";
import { SupportUsWays } from "@/components/marketing/support-us/support-us-ways";
import { SupportUsSponsor } from "@/components/marketing/support-us/support-us-sponsor";
import { pageMetadata } from "@/lib/seo/page-metadata";

type SupportUsPageProps = {
  params: Promise<{ locale: string }>;
};

export async function generateMetadata({
  params,
}: SupportUsPageProps): Promise<Metadata> {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: "SupportUs.meta" });
  return pageMetadata({
    locale,
    path: "/support-us",
    title: t("title"),
    description: t("description"),
  });
}

export default async function SupportUsPage({ params }: SupportUsPageProps) {
  const { locale } = await params;
  setRequestLocale(locale);
  const t = await getTranslations("SupportUs.header");

  return (
    <main className="mx-auto max-w-[1280px] px-4 rail:px-8">
      <SitePageHeader eyebrow={t("eyebrow")} title={t("title")} lead={t("lead")} />
      <SupportUsWays />
      <SupportUsSponsor />
    </main>
  );
}