/**
 * One block per release, newest first. ⚠️ Each block's id is its version, an announcement's
 * `link_url` target.
 */

import { getLocale, getTranslations } from "next-intl/server";

import { ChevronDownIcon } from "@/components/shared/icon";
import { ChangelogFilter } from "./changelog-filter";
import { HIDDEN_ON } from "./changelog-filter-values";
import { releaseView } from "./release-view";
import { RELEASES } from "./releases";
import { SITE_GRID_CLASS } from "@/components/marketing/site-grid";

const KIND_CLASS = {
  features: "border-dash-accent text-dash-accent",
  fixes: "border-dash-warn text-dash-warn",
} as const;

export async function ChangelogList() {
  const t = await getTranslations("Changelog");
  const locale = await getLocale();
  // ⚠️ An ISO date parses as UTC midnight: read in UTC, or west of it shows the day before.
  const longDate = new Intl.DateTimeFormat(locale, { dateStyle: "long", timeZone: "UTC" });
  const shortDate = new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeZone: "UTC" });

  const releases = RELEASES.map((release) => ({ release, view: releaseView(release) }));
  const features = releases.reduce((n, { view }) => n + view.features.length, 0);
  const fixes = releases.reduce((n, { view }) => n + view.fixes.length, 0);
  const first = RELEASES[RELEASES.length - 1];

  return (
    <ChangelogFilter
      counts={{ all: features + fixes, features, fixes }}
      caption={t("caption", {
        count: RELEASES.length,
        date: first ? shortDate.format(new Date(first.date)) : "",
      })}
    >
      {releases.map(({ release, view }, index) => (
        <article
          key={release.version}
          id={release.version}
          aria-labelledby={`${release.version}-title`}
          className={`${SITE_GRID_CLASS} gap-y-4 border-b border-dash-rule py-7 rail:py-10 ${view.features.length === 0 ? HIDDEN_ON.features : ""} ${view.fixes.length === 0 ? HIDDEN_ON.fixes : ""}`}
        >
          <div className="col-span-12 flex flex-row flex-wrap items-baseline gap-x-4 gap-y-2 wide:col-span-3 wide:flex-col wide:items-start">
            <h2
              id={`${release.version}-title`}
              className="font-dash-mono text-[28px] leading-[1.1] font-medium tracking-[-0.02em]"
            >
              {release.version}
            </h2>
            <time dateTime={release.date} className="text-[14px] text-dash-ink-2">
              {longDate.format(new Date(release.date))}
            </time>
            {index === 0 && (
              <span className="bg-dash-accent px-1.5 py-0.5 font-dash-mono text-[11px] font-semibold tracking-[0.1em] text-dash-surface uppercase">
                {t("latest")}
              </span>
            )}
          </div>

          <div className="col-span-12 flex min-w-0 flex-col gap-5.5 wide:col-span-9">
            <p className="text-[18px] font-medium tracking-[-0.01em]">
              {view.internalOnly ? t("internalOnly") : release.summary}
            </p>
            {view.features.length > 0 && (
              <ItemGroup
                label={t("sections.features")}
                kindClass={KIND_CLASS.features}
                items={view.features}
                className={HIDDEN_ON.fixes}
              />
            )}
            {view.fixes.length > 0 && (
              <ItemGroup
                label={t("sections.fixes")}
                kindClass={KIND_CLASS.fixes}
                items={view.fixes}
                className={HIDDEN_ON.features}
              />
            )}
            {view.technical.length > 0 && (
              <details className={`group/tech ${HIDDEN_ON.features} ${HIDDEN_ON.fixes}`}>
                <summary className="inline-flex min-h-11 cursor-pointer list-none items-center gap-2 font-dash-mono text-[12px] text-dash-ink-3 hover:text-dash-ink group-open/tech:text-dash-ink rail:min-h-9 [&::-webkit-details-marker]:hidden">
                  {t("technical", { count: view.technical.length })}
                  <ChevronDownIcon size={12} className="shrink-0 transition-transform group-open/tech:rotate-180" />
                </summary>
                <ItemList items={view.technical} className="mt-1.5 text-[14px] text-dash-ink-3" />
              </details>
            )}
          </div>
        </article>
      ))}
    </ChangelogFilter>
  );
}

function ItemGroup({
  label,
  kindClass,
  items,
  className,
}: {
  label: string;
  kindClass: string;
  items: readonly string[];
  className: string;
}) {
  return (
    <div className={`flex flex-col gap-0.5 ${className}`}>
      <h3 className="mb-1.5">
        <span
          className={`inline-block border px-1.5 py-0.5 font-dash-mono text-[11px] font-semibold tracking-[0.1em] uppercase ${kindClass}`}
        >
          {label}
        </span>
      </h3>
      <ItemList items={items} className="text-dash-ink-2" />
    </div>
  );
}

function ItemList({ items, className }: { items: readonly string[]; className: string }) {
  return (
    <ul className={`border-t border-dash-rule ${className}`}>
      {items.map((item, index) => (
        <li
          key={index}
          className="grid grid-cols-[20px_minmax(0,1fr)] gap-2 border-b border-dash-rule py-2.5"
        >
          <span aria-hidden="true" className="text-dash-ink-3">
            —
          </span>
          <span>{item}</span>
        </li>
      ))}
    </ul>
  );
}
