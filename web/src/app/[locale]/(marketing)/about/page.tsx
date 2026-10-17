/** The About page: what Yog-Scope is, its approach, its scope and who is behind it. Fully static. */

import { setRequestLocale, getTranslations } from "next-intl/server";
import type { Metadata } from "next";

import { AboutHero } from "@/components/marketing/about/about-hero";
import { AboutSections } from "@/components/marketing/about/about-sections";

type AboutPageProps = {
  params: Promise<{ locale: string }>;
};

export async function generateMetadata({
  params,
}: AboutPageProps): Promise<Metadata> {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: "About.meta" });
  return {
    title: t("title"),
    description: t("description"),
  };
}

export default async function AboutPage({ params }: AboutPageProps) {
  const { locale } = await params;
  setRequestLocale(locale);

  return (
    <main className="mx-auto max-w-[1280px] px-4 rail:px-8">
      <AboutHero />
      <AboutSections />
    </main>
  );
}
