/** The changelog's header; the newest release below carries its own date. */

import { getTranslations } from "next-intl/server";

import { SITE_LABEL_CLASS } from "@/components/marketing/site-label";

export async function ChangelogHeader() {
  const t = await getTranslations("Changelog.header");

  return (
    <header className="grid grid-cols-12 items-end gap-x-8 gap-y-5 pt-12 pb-8 rail:pt-[88px] rail:pb-10">
      <div className="col-span-12 flex flex-col gap-4.5 wide:col-span-7">
        <span className={SITE_LABEL_CLASS}>{t("eyebrow")}</span>
        <h1 className="text-[42px] leading-none font-semibold tracking-[-0.035em] rail:text-[64px]">
          {t("title")}
        </h1>
      </div>
      <p className="col-span-12 text-[17px] text-dash-ink-2 wide:col-span-5">
        {t("lead")}
      </p>
    </header>
  );
}
