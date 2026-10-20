"use client";

/**
 * The foldable Preferences at the foot of the menu: closed it shows the active language, open
 * it offers FR | EN, each a link to the same page and query in that language. The collapsed
 * rail keeps only their icon.
 */

import { useId } from "react";
import { useSearchParams } from "next/navigation";
import { useLocale, useTranslations } from "next-intl";

import { ChevronDownIcon, PreferencesIcon } from "@/components/shared/icon";
import { LocaleSwitch } from "@/components/shared/locale-switch";
import { usePathname } from "@/i18n/navigation";

import { ICON_BOX } from "./sidebar-nav-list";

export function Preferences({
  open,
  onToggle,
  collapsed,
  folded,
}: {
  /** Owned by the shell, which also folds the rail. */
  open: boolean;
  onToggle: () => void;
  collapsed: boolean;
  /** The folded rail hides the panel whatever `open` says. */
  folded: boolean;
}) {
  const t = useTranslations("Dashboard.preferences");
  const locale = useLocale();
  const pathname = usePathname();
  const query = useSearchParams().toString();
  const href = query ? `${pathname}?${query}` : pathname;

  const panelId = useId();
  const labelId = useId();

  const hiddenWhenCollapsed = collapsed ? "rail:hidden" : "";

  return (
    <div className="border-t border-dash-rule pt-2">
      <button
        type="button"
        onClick={onToggle}
        aria-expanded={open && !folded}
        aria-controls={panelId}
        title={collapsed ? t("title") : undefined}
        className={`group flex min-h-11 w-full items-center justify-between gap-2 px-[10px] text-left text-[14px] transition-colors hover:bg-dash-surface hover:text-dash-ink rail:min-h-10 ${open ? "text-dash-ink" : "text-dash-ink-2"} ${collapsed ? "rail:justify-center rail:px-0" : ""}`}
      >
        <span className="flex items-center gap-[10px]">
          <span
            aria-hidden="true"
            className={`${ICON_BOX} text-dash-ink-3 group-hover:text-dash-ink`}
          >
            <PreferencesIcon size={16} />
          </span>
          <span className={collapsed ? "rail:sr-only" : undefined}>{t("title")}</span>
        </span>
        <span className={`inline-flex items-center gap-2 font-dash-mono text-[11px] text-dash-ink-3 uppercase ${hiddenWhenCollapsed}`}>
          {locale}
          <ChevronDownIcon
            size={12}
            className={`transition-transform ${open ? "" : "rotate-180"}`}
          />
        </span>
      </button>

      <div id={panelId} hidden={!open} className={hiddenWhenCollapsed}>
        <div className="flex min-h-11 items-center justify-between gap-3 pt-0.5 pr-[10px] pb-1 pl-10 text-[14px] text-dash-ink-2 rail:min-h-9">
          <span id={labelId}>{t("language")}</span>
          <LocaleSwitch
            href={href}
            labelledBy={labelId}
            sizeClassName="min-h-11 min-w-11 rail:min-h-[30px] rail:min-w-10"
          />
        </div>
      </div>
    </div>
  );
}
