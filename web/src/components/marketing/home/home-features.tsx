/** The four features, a Server Component. ⚠️ `id="features"` is the target of every "Features" link. */

import { useTranslations } from "next-intl";

import { HomeSectionHead } from "./home-section-head";
import { SeverityBadge } from "./severity-badge";

const FEATURE_KEYS = ["observe", "reconstruct", "analyze", "signal"] as const;

export function HomeFeatures() {
  const t = useTranslations("Marketing.features");

  return (
    <section
      id="features"
      aria-labelledby="features-title"
      className="pt-18 rail:pt-28"
    >
      <HomeSectionHead
        id="features-title"
        eyebrow={t("eyebrow")}
        title={t("title")}
        intro={t("intro")}
      />
      <div className="grid grid-cols-1 border-t border-l border-dash-rule rail:grid-cols-2 wide:grid-cols-4">
        {FEATURE_KEYS.map((key) => (
          <article
            key={key}
            className="flex min-w-0 flex-col gap-3.5 border-r border-b border-dash-rule bg-dash-surface px-6 pt-7 pb-7.5"
          >
            <h3 className="text-[22px] font-semibold tracking-[-0.015em]">
              {t(`items.${key}.title`)}
            </h3>
            <p className="text-[15px] text-dash-ink-2">{t(`items.${key}.body`)}</p>
            <div className="mt-auto border-t border-dash-rule pt-3.5 font-dash-mono text-[11px] leading-[1.7] text-dash-ink-3">
              {key === "signal" ? (
                <span className="flex flex-wrap gap-1">
                  <SeverityBadge severity="critical" />
                  <SeverityBadge severity="warning" />
                  <SeverityBadge severity="info" />
                </span>
              ) : (
                t.rich(`items.${key}.events`, { br: () => <br /> })
              )}
            </div>
          </article>
        ))}
      </div>
    </section>
  );
}
