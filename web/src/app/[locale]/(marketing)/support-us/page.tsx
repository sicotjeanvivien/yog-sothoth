/** The support page, from the cheapest help to the most engaged: visibility, feedback, sponsoring. */

import { setRequestLocale, getTranslations } from "next-intl/server";
import type { Metadata } from "next";

import { SupportUsHeader } from "@/components/marketing/support-us/support-us-header";
import { SupportUsMakeKnown } from "@/components/marketing/support-us/support-us-make-known";
import { SupportUsActions } from "@/components/marketing/support-us/support-us-actions";

type SupportUsPageProps = {
  params: Promise<{ locale: string }>;
};

export async function generateMetadata({
  params,
}: SupportUsPageProps): Promise<Metadata> {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: "SupportUs.meta" });
  return {
    title: t("title"),
    description: t("description"),
  };
}

export default async function SupportUsPage({ params }: SupportUsPageProps) {
  const { locale } = await params;
  setRequestLocale(locale);

  return (
    <main>
      <SupportUsHeader />
      <SupportUsMakeKnown />
      <SupportUsActions />
    </main>
  );
}