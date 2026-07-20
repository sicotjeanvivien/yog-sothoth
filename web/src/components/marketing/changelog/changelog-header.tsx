/**
 * Changelog — header section.
 *
 * Sober introductory block: eyebrow, title, lead. Mirrors
 * `TermsHeader`/`PrivacyHeader` for visual consistency across the
 * marketing pages. No last-updated line — the newest release block
 * right below carries its own date.
 */

import { getTranslations } from "next-intl/server";

export async function ChangelogHeader() {
  const t = await getTranslations("Changelog.header");

  return (
    <section className="mx-auto max-w-[1800px] px-6 pt-20 pb-12 lg:px-12 lg:pt-28 lg:pb-16">
      <div className="mx-auto max-w-[128ch]">
        <p className="text-[14px] font-semibold tracking-[0.28em] text-slate-400 uppercase">
          {t("eyebrow")}
        </p>
        <h1 className="mt-4 font-display text-[36px] leading-[1.15] font-bold tracking-[0.04em] text-[#f5f2ff] [text-shadow:0_0_36px_rgba(139,92,246,0.4)] lg:text-[48px]">
          {t("title")}
        </h1>
        <p className="mt-6 text-[18px] leading-[1.7] text-slate-300">
          {t("lead")}
        </p>
      </div>
    </section>
  );
}
