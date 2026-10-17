/** The homepage's closing call to action, before the footer. */

import { useTranslations } from "next-intl";
import { DashboardButton } from "@/components/shared/dashboard-button";
import { LogoMark } from "@/components/shared/logo-mark";

export function HomeCta() {
  const t = useTranslations("Marketing.cta");

  return (
    <section className="mx-auto max-w-[1800px] px-6 lg:px-12">
      <div className="relative overflow-hidden rounded-[10px] border border-sothoth-500/20 bg-cosmos-900/60 px-6 py-6 text-center">
        <div
          aria-hidden="true"
          className="pointer-events-none absolute inset-0 bg-[radial-gradient(ellipse_50%_70%_at_50%_50%,rgba(124,58,237,0.22),transparent_70%)]"
        />

        <div className="relative z-1 flex flex-col items-center">
          <LogoMark width={64} />

          <h2 className="mt-6 font-display text-[28px] font-bold tracking-[0.04em] text-[#f5f2ff] lg:text-[36px]">
            {t("title")}
          </h2>

          <p className="mt-3 max-w-115 text-[17px] leading-[1.65] text-slate-400">
            {t("subtitle")}
          </p>

          <div className="mt-8">
            <DashboardButton />
          </div>
        </div>
      </div>
    </section>
  );
}
