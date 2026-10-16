/** The terms of service, between Yog-Scope's editor and any visitor of the site or the API. */

import { setRequestLocale, getTranslations } from "next-intl/server";
import type { Metadata } from "next";

import { TermsHeader } from "@/components/marketing/terms/terms-header";
import { TermsProse } from "@/components/marketing/terms/terms-prose";

type TermsPageProps = {
  params: Promise<{ locale: string }>;
};

export async function generateMetadata({
  params,
}: TermsPageProps): Promise<Metadata> {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: "Terms.meta" });
  return {
    title: t("title"),
    description: t("description"),
  };
}

export default async function TermsPage({ params }: TermsPageProps) {
  const { locale } = await params;
  setRequestLocale(locale);

  return (
    <main>
      <TermsHeader />
      <TermsProse />
    </main>
  );
}