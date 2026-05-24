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
import type { FC } from "react";

import {
  EyeIcon,
  PoolsIcon,
  PulseIcon,
  GithubIcon,
  type IconProps,
} from "@/components/shared/icon";
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

// ── Principles configuration ──────────────────────────────────────────
//
// Mirrors the `PILLARS` table in `HomePillars` so the two pages
// share a coherent visual vocabulary. Keys map to the
// `About.principles.items` i18n namespace.

type Principle = {
  key: string;
  icon: FC<IconProps>;
  /** Icon-badge classes — text colour + translucent fill + border. */
  accent: string;
};

const PRINCIPLES: readonly Principle[] = [
  {
    key: "openSource",
    icon: GithubIcon,
    accent: "text-sothoth-400 bg-sothoth-600/15 border-sothoth-500/25",
  },
  {
    key: "realTime",
    icon: PulseIcon,
    accent: "text-eldritch-400 bg-eldritch-500/15 border-eldritch-500/25",
  },
  {
    key: "protocolCentric",
    icon: PoolsIcon,
    accent: "text-signal-good bg-signal-good/15 border-signal-good/25",
  },
  {
    key: "empirical",
    icon: EyeIcon,
    accent: "text-sothoth-400 bg-sothoth-600/15 border-sothoth-500/25",
  },
] as const;

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
