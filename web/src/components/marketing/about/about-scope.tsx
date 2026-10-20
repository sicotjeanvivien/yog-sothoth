/** The About page's scope: the dashboard menu's protocols, each one observed or "Soon". */

import { useTranslations } from "next-intl";

import { PROTOCOL_SCOPE } from "@/components/dashboard/sidebar/sidebar-nav";
import type { PlatformKey, ProductKey } from "@/components/dashboard/sidebar/sidebar-keys";
import { SITE_LABEL_CLASS } from "@/components/marketing/site-label";

/** Ingested today, whether or not the dashboard has its page yet. */
const OBSERVED: ReadonlySet<ProductKey | PlatformKey> = new Set(["meteoraDammV2"]);

const TAG_CLASS =
  "px-[7px] py-0.5 font-dash-mono text-[10px] font-semibold tracking-[0.1em] whitespace-nowrap uppercase";

export function AboutScope() {
  const t = useTranslations("About.sections.availability");
  const tNav = useTranslations("Dashboard.Sidebar.nav");

  return (
    <div>
      <span className={SITE_LABEL_CLASS}>{t("scope")}</span>
      <ul className="mt-2.5 border-t border-dash-rule">
        {PROTOCOL_SCOPE.map((entry) => {
          const observed = OBSERVED.has(entry.key);
          return (
            <li
              key={entry.key}
              className="flex items-center justify-between gap-4 border-b border-dash-rule py-3"
            >
              <span className={observed ? undefined : "text-dash-ink-3"}>
                {entry.labelKeys.map((key) => tNav(key)).join(" ")}
              </span>
              {observed ? (
                <span className={`${TAG_CLASS} bg-dash-accent text-dash-surface`}>
                  {t("observed")}
                </span>
              ) : (
                <span
                  className={`${TAG_CLASS} border border-dashed border-dash-ink-3 text-dash-ink-3`}
                >
                  {tNav("soon")}
                </span>
              )}
            </li>
          );
        })}
      </ul>
    </div>
  );
}
