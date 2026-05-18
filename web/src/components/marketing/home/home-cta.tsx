/**
 * Homepage closing CTA section.
 *
 * A centered call-to-action band — the last invitation to click
 * before the footer. Logo, title, one-line sub-text, a single
 * button.
 *
 * The band is a bordered box on a slightly distinct surface, with a
 * soft violet radial glow behind the content (pure CSS — no image
 * asset). Sits in the page flow between the feature pillars and the
 * footer.
 *
 * Static content, Server Component.
 */

import Image from "next/image";
import { useTranslations } from "next-intl";
import { DashboardButton } from "@/components/shared/dashboard-button";

export function HomeCta() {
  const t = useTranslations("Marketing.cta");

  return (
    <section className="mx-auto max-w-[1800px] px-6 lg:px-12">
      {/* The band — bordered box, relative so the glow can sit behind. */}
      <div className="relative overflow-hidden rounded-[10px] border border-sothoth-500/20 bg-cosmos-900/60 px-6 py-6 text-center">
        {/* Violet radial glow, centered behind the content. */}
        <div
          aria-hidden="true"
          className="pointer-events-none absolute inset-0 bg-[radial-gradient(ellipse_50%_70%_at_50%_50%,rgba(124,58,237,0.22),transparent_70%)]"
        />

        {/* Content, above the glow. */}
        <div className="relative z-[1] flex flex-col items-center">
          <Image
            src="/logo.png"
            alt=""
            width={64}
            height={64}
            className="h-[64px] w-[64px] object-contain [filter:drop-shadow(0_0_16px_rgba(139,92,246,0.55))]"
          />

          <h2 className="mt-6 font-display text-[28px] font-bold tracking-[0.04em] text-[#f5f2ff] lg:text-[36px]">
            {t("title")}
          </h2>

          <p className="mt-3 max-w-[460px] text-[17px] leading-[1.65] text-slate-400">
            {t("subtitle")}
          </p>

          <div className="mt-8">
            <DashboardButton size="lg" />
          </div>
        </div>
      </div>
    </section>
  );
}
