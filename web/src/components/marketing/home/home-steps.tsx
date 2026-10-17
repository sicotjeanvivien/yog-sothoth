/**
 * "How it works": five numbered steps on one line, joined by a rule; up to 1100 px, a
 * column with the rule down the left.
 */

import { useTranslations } from "next-intl";

import { HomeSectionHead } from "./home-section-head";

const STEP_KEYS = ["listen", "decode", "enrich", "store", "expose"] as const;

export function HomeSteps() {
  const t = useTranslations("Marketing.steps");

  return (
    <section aria-labelledby="steps-title" className="pt-18 rail:pt-28">
      <HomeSectionHead
        id="steps-title"
        eyebrow={t("eyebrow")}
        title={t("title")}
        intro={t("intro")}
      />
      <ol className="relative grid grid-cols-1 gap-7 before:absolute before:inset-y-0 before:left-1.5 before:w-px before:bg-linear-to-b before:from-dash-accent before:to-dash-rule wide:grid-cols-5 wide:gap-6 wide:pt-1 wide:before:inset-x-0 wide:before:top-2.5 wide:before:bottom-auto wide:before:h-px wide:before:w-auto wide:before:bg-linear-to-r">
        {STEP_KEYS.map((key, index) => (
          <li
            key={key}
            className="relative flex min-w-0 flex-col gap-2.5 pl-9 wide:pl-0"
          >
            <span
              aria-hidden="true"
              className={`absolute top-0.75 left-0 z-1 size-3 border border-dash-accent wide:relative wide:top-auto wide:mb-2 ${index === 0 ? "bg-dash-accent" : "bg-dash-bg"}`}
            />
            <span className="font-dash-mono text-[12px] tracking-[0.08em] text-dash-accent">
              {String(index + 1).padStart(2, "0")}
            </span>
            <h3 className="text-[17px] font-semibold">{t(`items.${key}.title`)}</h3>
            <p className="text-[14px] text-dash-ink-2">{t(`items.${key}.body`)}</p>
          </li>
        ))}
      </ol>
    </section>
  );
}
