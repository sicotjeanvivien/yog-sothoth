"use client";

/** The dot beside an open protocol in the menu: green while the ingestion is live, else nothing. */

import { useTranslations } from "next-intl";

import type { LiveState } from "@/components/dashboard/live-status/live-status";

export function ProductLiveDot({ state }: { state: LiveState }) {
  const t = useTranslations("Dashboard.liveStatus.states");
  if (state !== "live") {
    return null;
  }
  return (
    <span className="inline-flex items-center">
      <span aria-hidden="true" className="h-2 w-2 bg-dash-up" />
      <span className="sr-only">{t("live")}</span>
    </span>
  );
}
