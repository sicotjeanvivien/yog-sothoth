/** The marketing homepage, `/[locale]`, fully static: not one API call. */

import { setRequestLocale } from "next-intl/server";

import { HomeHero } from "@/components/marketing/home/home-hero";
import { HomeFacts } from "@/components/marketing/home/home-facts";
import { HomeFeatures } from "@/components/marketing/home/home-features";
import { HomeSteps } from "@/components/marketing/home/home-steps";
import { HomeCta } from "@/components/marketing/home/home-cta";

type HomePageProps = {
  params: Promise<{ locale: string }>;
};

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
