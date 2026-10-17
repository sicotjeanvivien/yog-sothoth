"use client";

/**
 * The foldable Preferences at the foot of the menu: closed it shows the active language, open
 * it offers FR | EN, each a link to the same page and query in that language.
 */

import { useId, useState } from "react";
import { useSearchParams } from "next/navigation";
import { useLocale, useTranslations } from "next-intl";

import { ChevronDownIcon } from "@/components/shared/icon";
import { LocaleSwitch } from "@/components/shared/locale-switch";
import { usePathname } from "@/i18n/navigation";

import {
  PREFERENCES_COOKIE_MAX_AGE_S,
  PREFERENCES_OPEN_COOKIE,
} from "./preferences-state";

export function Preferences({ initialOpen }: { initialOpen: boolean }) {
  const t = useTranslations("Dashboard.preferences");
  const locale = useLocale();
  const pathname = usePathname();
  const query = useSearchParams().toString();
  const href = query ? `${pathname}?${query}` : pathname;

  const [open, setOpen] = useState(initialOpen);
  const panelId = useId();
  const labelId = useId();

  const toggle = () => {
    const next = !open;
    document.cookie = `${PREFERENCES_OPEN_COOKIE}=${next ? "1" : "0"}; path=/; max-age=${PREFERENCES_COOKIE_MAX_AGE_S}; samesite=lax`;
    setOpen(next);
  };

  return (
    <div className="border-t border-dash-rule pt-2">
      <button
        type="button"
        onClick={toggle}
        aria-expanded={open}
        aria-controls={panelId}
        className={`flex min-h-11 w-full items-center justify-between gap-2 px-[10px] text-left text-[14px] transition-colors hover:bg-dash-surface hover:text-dash-ink rail:min-h-10 ${open ? "text-dash-ink" : "text-dash-ink-2"}`}
      >
        <span>{t("title")}</span>
        <span className="inline-flex items-center gap-2 font-dash-mono text-[11px] text-dash-ink-3 uppercase">
          {locale}
          <ChevronDownIcon
            size={12}
            className={`transition-transform ${open ? "" : "rotate-180"}`}
          />
        </span>
      </button>

      <div id={panelId} hidden={!open}>
        <div className="flex min-h-11 items-center justify-between gap-3 pt-0.5 pr-[10px] pb-1 pl-[22px] text-[14px] text-dash-ink-2 rail:min-h-9">
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
