/**
 * The homepage hero: copy over the left half of a full-bleed artwork composed empty on the
 * left, under a light scrim. A Server Component.
 */

import Image from "next/image";
import { useTranslations } from "next-intl";
import { DashboardButton } from "@/components/shared/dashboard-button";
import { SolanaGlyph } from "@/components/shared/icon";
import { CtaLink } from "@/components/shared/cta-link";

export function HomeHero() {
  const t = useTranslations("Marketing.hero");

  return (
    <section className="relative flex min-h-[86vh] items-center overflow-hidden">
      <Image
        src="/hero-visual.png"
        alt=""
        fill
        priority
        sizes="100vw"
        className="object-cover object-center"
      />

      <div className="absolute inset-0 bg-[linear-gradient(90deg,rgba(5,3,13,0.80)_0%,rgba(5,3,13,0.55)_30%,rgba(5,3,13,0.15)_50%,transparent_66%)]" />

      <div className="relative z-1 mx-auto w-full max-w-[1800px] px-6 lg:px-12">
        <div className="max-w-full lg:max-w-[50%]">
          <h1 className="font-display text-[36px] leading-[1.13] font-bold tracking-[0.04em] text-[#f5f2ff] [text-shadow:0_0_36px_rgba(139,92,246,0.4)] lg:text-[52px]">
            {t("title")}
          </h1>

          <p className="mt-4 font-display text-[20px] font-medium tracking-[0.34em] text-slate-400 uppercase indent-[0.34em]">
            {t("subtitle")}
          </p>

          <p className="mt-6 max-w-105 text-[17px] leading-[1.7] text-slate-300">
            {t("lead")}
          </p>

          <div className="mt-8.5 flex items-center gap-3.5">
            <DashboardButton size="lg" />
            <CtaLink href="#features" label={t("ctaSecondary")} />
          </div>

          <div className="mt-8 flex items-center gap-2.25 text-[11px] font-semibold tracking-[0.16em] text-slate-500 uppercase">
            <SolanaGlyph />
            {t("builtOn")}
          </div>
        </div>
      </div>
    </section>
  );
}
