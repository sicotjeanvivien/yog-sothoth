"use client";

/**
 * The beta warning, shown by the server layout until "Got it"; this island only dismisses.
 * In the flow, not sticky: a warning read once, not an alert.
 */

import { useState } from "react";
import { useTranslations } from "next-intl";

import { BetaBadge } from "./beta-badge";
import { REPORT_ERROR_HREF, rememberBetaDismissed } from "./beta-state";

export function BetaBanner() {
  const t = useTranslations("Dashboard.Beta");
  const [dismissed, setDismissed] = useState(false);

  if (dismissed) return null;

  const dismiss = () => {
    rememberBetaDismissed();
    setDismissed(true);
  };

  return (
    <div
      role="note"
      className="flex items-center gap-x-4 gap-y-2 border-b border-dash-rule bg-dash-surface-2 px-8 py-2.5 font-dash-sans text-[12px] leading-normal text-dash-ink-2 max-rail:flex-wrap max-rail:px-4"
    >
      <BetaBadge className="shrink-0" />
      <p className="min-w-0 flex-[1_1_320px]">
        <strong className="font-medium text-dash-ink">{t("lead")}</strong>{" "}
        {t("body")}{" "}
        <a href={REPORT_ERROR_HREF} className="text-dash-accent">
          {t("report")}
        </a>
      </p>
      <button
        type="button"
        onClick={dismiss}
        className="shrink-0 border border-dash-rule px-2.5 py-1 text-[12px] text-dash-ink-2 transition-colors hover:border-dash-ink-3 hover:text-dash-ink"
      >
        {t("dismiss")}
      </button>
    </div>
  );
}
