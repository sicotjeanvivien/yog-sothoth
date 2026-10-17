/** Yog-Scope in four facts, under the hero. */

import { useTranslations } from "next-intl";

import { SITE_LABEL_CLASS } from "@/components/marketing/site-label";

const FACT_KEYS = ["capture", "history", "signals", "access"] as const;

export function HomeFacts() {
  const t = useTranslations("Marketing.facts");

  return (
    <section aria-label={t("label")}>
      <dl className="grid grid-cols-1 border-t border-l border-dash-rule rail:grid-cols-2 wide:grid-cols-4">
        {FACT_KEYS.map((key) => (
          <div
            key={key}
            className="flex flex-col gap-1.5 border-r border-b border-dash-rule bg-dash-surface px-6 py-5.5"
          >
            <dt className={SITE_LABEL_CLASS}>{t(`${key}.label`)}</dt>
            <dd className="text-[20px] font-semibold tracking-[-0.01em]">
              {t(`${key}.value`)}
            </dd>
          </div>
        ))}
      </dl>
    </section>
  );
}
