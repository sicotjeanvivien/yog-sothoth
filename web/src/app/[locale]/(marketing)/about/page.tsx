/**
 * About page.
 *
 * Editorial page describing what Yog-Sothoth is, the principles it
 * is built on, its current scope, and who is behind it. Strictly
 * factual — no marketing copy.
 *
 * Layout
 *
 *   1. Hero — split: copy left, logo with glow right (stacks on
 *      mobile). Reuses `/logo.png`; no new asset is needed.
 *   2. Principles — four cards in a responsive grid, mirroring the
 *      pillar-card vocabulary from `HomePillars` for visual
 *      consistency.
 *   3. At a glance + Current focus — two-column band. Left is a
 *      key/value card listing project facts; right is a short note
 *      on the current stage.
 *   4. Behind the project — bordered band with awsd.fr + GitHub
 *      links, in the spirit of `HomeCta`.
 *
 * Static content, Server Component. Copy lives under the `About`
 * namespace in `messages/{en,fr}.json`.
 */

import { setRequestLocale, getTranslations } from "next-intl/server";
import type { Metadata } from "next";

import { AboutHero } from "@/components/marketing/about/about-hero";
import { AboutProse } from "@/components/marketing/about/about-prose";

// ── Page metadata ─────────────────────────────────────────────────────

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

// ── Page ──────────────────────────────────────────────────────────────

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
