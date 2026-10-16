/** The marketing homepage, `/[locale]`, fully static. */

import { setRequestLocale } from "next-intl/server";

import { HomeHero } from "@/components/marketing/home/home-hero";
import { HomePillars } from "@/components/marketing/home/home-pillars";
import { HomeCta } from "@/components/marketing/home/home-cta";

type HomePageProps = {
  params: Promise<{ locale: string }>;
};

export default async function HomePage({ params }: HomePageProps) {
  const { locale } = await params;
  setRequestLocale(locale);

  return (
    <main>
      <HomeHero />
      <HomePillars />
      <HomeCta />
    </main>
  );
}