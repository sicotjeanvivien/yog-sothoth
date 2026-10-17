/** The homepage hero: the pitch and its two buttons, beside the dashboard preview. */

import { useTranslations } from "next-intl";

import { DashboardButton } from "@/components/shared/dashboard-button";
import { SecondaryButton } from "@/components/shared/secondary-button";
import { SITE_GRID_CLASS } from "@/components/marketing/site-grid";
import { SITE_LABEL_CLASS } from "@/components/marketing/site-label";
import { HomePreview } from "./home-preview";

const META_KEYS = ["chain", "design", "access"] as const;

export function HomeHero() {
  const t = useTranslations("Marketing.hero");

  return (
    <section
      aria-labelledby="hero-title"
      className={`${SITE_GRID_CLASS} items-center gap-y-12 pt-12 pb-14 rail:pt-[88px] rail:pb-24`}
    >
      <div className="col-span-12 flex flex-col gap-6 wide:col-span-7">
        <span className={`${SITE_LABEL_CLASS} inline-flex items-center gap-2`}>
          <span aria-hidden="true" className="size-2 shrink-0 bg-dash-up" />
          {t("label")}
        </span>
        <h1
          id="hero-title"
          className="text-[44px] leading-none font-semibold tracking-[-0.035em] rail:text-[72px]"
        >
          {t.rich("title", {
            accent: (chunks) => <em className="text-dash-accent not-italic">{chunks}</em>,
          })}
        </h1>
        <p className="max-w-[54ch] text-[16px] leading-[1.55] text-dash-ink-2 rail:text-[18px]">
          {t("lead")}
        </p>
        <div className="flex flex-wrap gap-3">
          <DashboardButton arrow />
          <SecondaryButton href="/#features" label={t("ctaSecondary")} />
        </div>
        <ul className="flex flex-wrap gap-x-5 gap-y-2 font-dash-mono text-[12px] text-dash-ink-3">
          {META_KEYS.map((key) => (
            <li key={key}>{t(`meta.${key}`)}</li>
          ))}
        </ul>
      </div>

      <div className="col-span-12 min-w-0 wide:col-span-5">
        <HomePreview />
      </div>
    </section>
  );
}
