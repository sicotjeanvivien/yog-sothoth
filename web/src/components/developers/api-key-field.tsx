"use client";

/** The visitor's API key, typed once for every route; kept in memory, so a reload clears it. */

import { useState } from "react";
import { useTranslations } from "next-intl";

import { GhostButton } from "./buttons";

export function ApiKeyField({ value, onChange }: { value: string; onChange: (value: string) => void }) {
  const t = useTranslations("Developers.key");
  const [shown, setShown] = useState(false);

  return (
    <div className="border-b border-dash-rule bg-dash-surface px-4 py-3 rail:border-l rail:px-8">
      <label className="flex flex-wrap items-center gap-x-3 gap-y-2">
        <span className="font-dash-mono text-[11px] font-medium tracking-[0.12em] text-dash-ink-3 uppercase">
          {t("label")}
        </span>
        <input
          type={shown ? "text" : "password"}
          value={value}
          onChange={(event) => onChange(event.target.value.trim())}
          placeholder={t("placeholder")}
          autoComplete="off"
          spellCheck={false}
          className="min-h-9 min-w-0 flex-1 border border-dash-rule bg-dash-bg px-2.5 font-dash-mono text-[12px] text-dash-ink outline-none placeholder:text-dash-ink-3 focus:border-dash-accent"
        />
        <GhostButton onClick={() => setShown((s) => !s)} aria-pressed={shown}>
          {shown ? t("hide") : t("show")}
        </GhostButton>
      </label>
      <p className="mt-1.5 text-[12px] text-dash-ink-3">{t("hint")}</p>
    </div>
  );
}
