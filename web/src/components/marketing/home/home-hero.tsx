/**
 * Homepage hero section.
 *
 * A full-bleed background image with the copy overlaid on the left
 * half. Three stacked layers:
 *
 *   1. the artwork (`next/image` with `fill`), composed empty-left
 *      so the copy has a calm dark area to sit on;
 *   2. a light scrim — a left-to-right gradient that only nudges
 *      contrast, since the image's left side is already near-black;
 *   3. the copy itself, capped at half the page width so it never
 *      crosses the mid-line into the artwork.
 *
 * Static content — no state, no data. Server Component.
 *
 * The hero image lives at `web/public/hero-visual.png`.
 */

import Image from "next/image";
import { useTranslations } from "next-intl";
import { DashboardButton } from "@/components/shared/dashboard-button";
import { SolanaGlyph } from "@/components/shared/icon";

export function HomeHero() {
  const t = useTranslations("Marketing.hero");

  return (
    <section className="relative flex min-h-[86vh] items-center overflow-hidden bg-cosmos-950">
      {/* Layer 1 — artwork */}
      <Image
        src="/hero-visual.png"
        alt=""
        fill
        priority
        sizes="100vw"
        className="object-cover object-center"
      />

      {/* Layer 2 — light scrim for legibility */}
      <div className="absolute inset-0 bg-[linear-gradient(90deg,rgba(5,3,13,0.80)_0%,rgba(5,3,13,0.55)_30%,rgba(5,3,13,0.15)_50%,transparent_66%)]" />

      {/* Layer 3 — copy */}
      <div className="relative z-[1] mx-auto w-full max-w-[1800px] px-6 lg:px-12">
        <div className="max-w-full lg:max-w-[50%]">
          <h1 className="font-display text-[36px] leading-[1.13] font-bold tracking-[0.04em] text-[#f5f2ff] [text-shadow:0_0_36px_rgba(139,92,246,0.4)] lg:text-[52px]">
            {t("title")}
          </h1>

          <p className="mt-4 font-display text-[20px] font-medium tracking-[0.34em] text-slate-400 uppercase [text-indent:0.34em]">
            {t("subtitle")}
          </p>

          <p className="mt-6 max-w-[420px] text-[17px] leading-[1.7] text-slate-300">
            {t("lead")}
          </p>

          <div className="mt-[34px] flex items-center gap-[14px]">
            <DashboardButton size="lg" />
            <HeroAnchorButton href="#features" label={t("ctaSecondary")} />
          </div>

          <div className="mt-8 flex items-center gap-[9px] text-[11px] font-semibold tracking-[0.16em] text-slate-500 uppercase">
            <SolanaGlyph />
            {t("builtOn")}
          </div>
        </div>
      </div>
    </section>
  );
}

// ── Sub-components ────────────────────────────────────────────────────

const BUTTON_CLASS =
  "inline-flex items-center gap-2 rounded-[4px] border border-sothoth-500/45 bg-sothoth-600/15 px-5 py-[11px] text-[17px] font-semibold text-[#f1ecff] transition-colors hover:border-sothoth-500/70 hover:bg-sothoth-600/30";

/**
 * Hero call-to-action pointing at an in-page anchor. A plain anchor
 * rather than next-intl's `Link` — `#features` is a fragment on the
 * current page, not a locale route, so it needs no locale prefix.
 */
function HeroAnchorButton({ href, label }: { href: string; label: string }) {
  return (
    <a href={href} className={BUTTON_CLASS}>
      {label}
    </a>
  );
}