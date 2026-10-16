/**
 * The About page hero: copy over the left half of a full-bleed artwork composed empty on the
 * left, under a light scrim. A Server Component.
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
     

      <div className="absolute inset-0 bg-[linear-gradient(90deg,rgba(5,3,13,0.80)_0%,rgba(5,3,13,0.55)_30%,rgba(5,3,13,0.15)_50%,transparent_66%)]" />

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