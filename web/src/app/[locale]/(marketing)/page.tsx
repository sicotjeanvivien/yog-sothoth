/** The marketing homepage, `/[locale]`, fully static: not one API call. */

import { getTranslations, setRequestLocale } from "next-intl/server";
import type { Metadata } from "next";

import { HomeHero } from "@/components/marketing/home/home-hero";
import { HomeFacts } from "@/components/marketing/home/home-facts";
import { HomeFeatures } from "@/components/marketing/home/home-features";
import { HomeSteps } from "@/components/marketing/home/home-steps";
import { HomeCta } from "@/components/marketing/home/home-cta";
import { pageMetadata } from "@/lib/seo/page-metadata";

type HomePageProps = {
  params: Promise<{ locale: string }>;
};

export async function generateMetadata({
  params,
}: HomePageProps): Promise<Metadata> {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: "Marketing.meta" });
  return pageMetadata({
    locale,
    path: "",
    title: t("title"),
    description: t("description"),
    absoluteTitle: true,
  });
}

export default async function HomePage({ params }: HomePageProps) {
  const { locale } = await params;
  setRequestLocale(locale);

  return (
    <main>
      <div className="mx-auto max-w-[1280px] px-4 rail:px-8">
        <HomeHero />
        <HomeFacts />
        <HomeFeatures />
        <HomeSteps />
      </div>
      <HomeCta />
    </main>
  );
}
