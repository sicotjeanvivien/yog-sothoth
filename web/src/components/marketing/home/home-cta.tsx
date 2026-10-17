/** The homepage's closing call to action, a full-width band before the footer. */

import { useTranslations } from "next-intl";

import { DashboardButton } from "@/components/shared/dashboard-button";
import { LogoMark } from "@/components/shared/logo-mark";
import { HOME_TITLE_CLASS } from "./home-section-head";

export function HomeCta() {
  const t = useTranslations("Marketing.cta");

  return (
    <section
      aria-labelledby="cta-title"
      className="mt-18 border-y border-dash-rule bg-dash-surface rail:mt-28"
    >
      <div className="mx-auto flex max-w-[1280px] flex-wrap items-center justify-between gap-x-12 gap-y-7 px-4 py-12 rail:px-8 rail:py-18">
        <div className="flex items-center gap-7">
          <LogoMark width={72} />
          <div>
            <h2 id="cta-title" className={HOME_TITLE_CLASS}>
              {t("title")}
            </h2>
            <p className="mt-2 max-w-[52ch] text-[15px] text-dash-ink-2">
              {t("subtitle")}
            </p>
          </div>
        </div>
        <DashboardButton arrow />
      </div>
    </section>
  );
}
