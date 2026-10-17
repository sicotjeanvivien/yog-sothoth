/** The About page hero: the pitch, its two buttons and the link down to "Who is behind it". */

import { useTranslations } from "next-intl";

import { DashboardButton } from "@/components/shared/dashboard-button";
import { SecondaryButton } from "@/components/shared/secondary-button";
import { SITE_GRID_CLASS } from "@/components/marketing/site-grid";
import { SITE_LABEL_CLASS } from "@/components/marketing/site-label";
import { BEHIND_ID, GITHUB_REPO_URL } from "./about-sections";

export function AboutHero() {
  const t = useTranslations("About.hero");

  return (
    <section
      aria-labelledby="about-title"
      className={`${SITE_GRID_CLASS} border-b border-dash-rule pt-12 pb-12 rail:pt-[88px] rail:pb-20`}
    >
      <div className="col-span-12 flex flex-col gap-6 wide:col-span-9">
        <span className={SITE_LABEL_CLASS}>{t("eyebrow")}</span>
        <h1
          id="about-title"
          className="text-[44px] leading-none font-semibold tracking-[-0.035em] rail:text-[72px]"
        >
          {t("title")} <span className="text-dash-ink-3">{t("titleMuted")}</span>
        </h1>
        <p className="max-w-[62ch] text-[16px] leading-[1.55] text-dash-ink-2 rail:text-[18px]">
          {t("lead")}
        </p>
        <div className="flex flex-wrap gap-3">
          <DashboardButton arrow />
          <SecondaryButton href={GITHUB_REPO_URL} label={t("ctaGithub")} external />
        </div>
        <a
          href={`#${BEHIND_ID}`}
          className="inline-flex min-h-11 items-center self-start text-[14px] text-dash-accent-2 hover:underline"
        >
          {t("whoLink")}
          <span aria-hidden="true">&nbsp;↓</span>
        </a>
      </div>
    </section>
  );
}
