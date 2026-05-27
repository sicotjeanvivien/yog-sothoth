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

import { useTranslations } from "next-intl";
import { GithubIcon } from "@/components/shared/icon";
import { CtaLink } from "@/components/shared/cta-link";

const GITHUB_REPO_URL = "https://github.com/sicotjeanvivien/yog-sothoth";
const AWSD_URL = "https://awsd.fr/";

export function AboutHero() {
  const t = useTranslations("About.hero");

  return (
    <section className="relative flex min-h-[86vh] items-center overflow-hidden">
      {/* Layer 1 — artwork */}
     

      {/* Layer 2 — light scrim for legibility */}
      <div className="absolute inset-0 bg-[linear-gradient(90deg,rgba(5,3,13,0.80)_0%,rgba(5,3,13,0.55)_30%,rgba(5,3,13,0.15)_50%,transparent_66%)]" />

      {/* Layer 3 — copy */}
      <div className="relative z-[1] mx-auto w-full max-w-[1800px] px-6 lg:px-12">
        <div className="max-w-full lg:max-w-[50%]">
          <p className="text-[14px] font-semibold tracking-[0.28em] text-slate-400 uppercase">
            {t("eyebrow")}
          </p>
          <h1 className="mt-4 font-display text-[40px] leading-[1.1] font-bold tracking-[0.04em] text-[#f5f2ff] [text-shadow:0_0_36px_rgba(139,92,246,0.4)] lg:text-[56px]">
            {t("titleLine1")}
            <br />
            <span className="text-sothoth-400">{t("titleLine2")}</span>
          </h1>

          <p className="mt-8 max-w-[520px] text-[19px] leading-[1.7] text-slate-300">
            {t("lead")}
          </p>

          <div className="mt-10 flex flex-wrap items-center gap-3">
            <CtaLink
              href={AWSD_URL}
              label={t("ctaAWSD")}
              variant="primary"
              external
            />
            <CtaLink
              href={GITHUB_REPO_URL}
              label={t("ctaGithub")}
              variant="primary"
              icon={<GithubIcon className="h-[16px] w-[16px]" />}
              external
            />
          </div>
        </div>
      </div>
    </section>
  );
}