/**
 * The About page: what Yog-Sothoth is, its principles, its scope and who is behind it — factual,
 * no marketing copy. Copy under `About` in `messages/{en,fr}/about.json`.
 */

import { setRequestLocale, getTranslations } from "next-intl/server";
import type { Metadata } from "next";

import { AboutHero } from "@/components/marketing/about/about-hero";
import { AboutProse } from "@/components/marketing/about/about-prose";

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
    <main>
      <AboutHero />
      <AboutProse />
    </main>
  );
}
