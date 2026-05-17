/**
 * Marketing homepage — `/[locale]`.
 *
 * Server Component, fully static. For now it is just the hero;
 * further sections (feature pillars, dashboard preview, CTA, footer)
 * will be added below it in follow-up commits.
 *
 * NOTE: merge this with your existing `(marketing)/page.tsx` —
 * it was a placeholder; this replaces the placeholder body with the
 * hero. Keep any locale params handling you already had.
 */

import { setRequestLocale } from "next-intl/server";

import { HomeHero } from "@/components/marketing/home/home-hero";
import { HomePillars } from "@/components/marketing/home/home-pillars";

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
    </main>
  );
}