"use client";

/**
 * The filter bar, and the frame that carries its choice as `data-filter`. ⚠️ The list inside
 * is server-rendered whole; its own `group-data-[filter=…]/changelog` classes hide by CSS.
 */

import { useState, type ReactNode } from "react";
import { useTranslations } from "next-intl";

import { SITE_LABEL_CLASS } from "@/components/marketing/site-label";
import { FILTERS, type ChangelogFilterValue } from "./changelog-filter-values";

export function ChangelogFilter({
  counts,
  caption,
  children,
}: {
  counts: Record<ChangelogFilterValue, number>;
  caption: string;
  children: ReactNode;
}) {
  const t = useTranslations("Changelog");
  const [filter, setFilter] = useState<ChangelogFilterValue>("all");

  return (
    <>
      <div className="flex flex-wrap items-center justify-between gap-x-6 gap-y-3 border-y border-dash-rule py-3.5">
        <div role="group" aria-label={t("filter.label")} className="inline-flex border border-dash-rule">
          {FILTERS.map((value) => {
            const on = value === filter;
            return (
              <button
                key={value}
                type="button"
                aria-pressed={on}
                onClick={() => setFilter(value)}
                className={`min-h-11 min-w-11 border-r border-dash-rule px-3.5 text-[13px] last:border-r-0 ${on ? "bg-dash-accent font-semibold text-dash-surface" : "text-dash-ink-2 hover:text-dash-ink"}`}
              >
                {t(value === "all" ? "filter.all" : `sections.${value}`)}
                <span className="ml-1.5 font-dash-mono text-[11px] opacity-80">
                  {counts[value]}
                </span>
              </button>
            );
          })}
        </div>
        <span className={SITE_LABEL_CLASS}>{caption}</span>
      </div>
      <div data-filter={filter} className="group/changelog">
        {children}
      </div>
    </>
  );
}
