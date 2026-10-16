/**
 * The pools search box, debounced, driving `q`. ⚠️ `replace`, not `push`: one history entry
 * per keystroke would break Back. A new query clears `cursor`/`dir`/`position`.
 */

"use client";

import { useEffect, useRef, useState } from "react";
import { useSearchParams } from "next/navigation";
import { useTranslations } from "next-intl";

import { useRouter, usePathname } from "@/i18n/navigation";
import { SearchIcon, CloseIcon } from "@/components/shared/icon"; // adapt if names differ
import { zListPoolsQuery } from "@/lib/api/schema/zod.gen";

const SEARCH_MAX_LENGTH = zListPoolsQuery.shape.q.unwrap().unwrap().maxLength ?? undefined;

const DEBOUNCE_MS = 300;

export function PoolsSearch() {
  const t = useTranslations("Dashboard.Pools.search");
  const router = useRouter();
  const pathname = usePathname();
  const searchParams = useSearchParams();

  const initialQuery = searchParams.get("q") ?? "";
  const [value, setValue] = useState(initialQuery);

  // Synced from the URL only when it differs from what was last pushed: no clobbering mid-typing.
  const lastPushedRef = useRef(initialQuery);
  useEffect(() => {
    const urlQuery = searchParams.get("q") ?? "";
    if (urlQuery !== lastPushedRef.current) {
      setValue(urlQuery);
      lastPushedRef.current = urlQuery;
    }
  }, [searchParams]);

  useEffect(() => {
    const handle = setTimeout(() => {
      const trimmed = value.trim();
      const current = searchParams.get("q") ?? "";

      // ⚠️ Without it, this and the sync effect above loop.
      if (trimmed === current) return;

      const params = new URLSearchParams(searchParams.toString());

      if (trimmed.length > 0) {
        params.set("q", trimmed);
      } else {
        params.delete("q");
      }

      params.delete("cursor");
      params.delete("dir");
      params.delete("position");

      lastPushedRef.current = trimmed;
      const qs = params.toString();
      router.replace(qs.length > 0 ? `${pathname}?${qs}` : pathname);
    }, DEBOUNCE_MS);

    return () => clearTimeout(handle);
  }, [value, searchParams, pathname, router]);

  const handleClear = () => {
    setValue("");
  };

  return (
    <div className="relative w-full lg:w-[320px]">
      <span
        className="pointer-events-none absolute inset-y-0 left-3 flex items-center text-slate-500"
        aria-hidden="true"
      >
        <SearchIcon className="h-4 w-4" />
      </span>

      <input
        type="search"
        inputMode="search"
        value={value}
        onChange={(e) => setValue(e.target.value)}
        placeholder={t("placeholder")}
        aria-label={t("ariaLabel")}
        maxLength={SEARCH_MAX_LENGTH}
        className="
          w-full rounded-md border border-sothoth-500/20 bg-cosmos-900/60
          py-2 pl-9 pr-9 text-sm text-slate-100 placeholder:text-slate-500
          transition-colors
          focus:border-sothoth-400/50 focus:bg-cosmos-900/90
          focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sothoth-400
        "
      />

      {value.length > 0 && (
        <button
          type="button"
          onClick={handleClear}
          aria-label={t("clear")}
          className="
            absolute inset-y-0 right-2 flex items-center text-slate-500
            transition-colors hover:text-slate-200
            focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sothoth-400
            rounded
          "
        >
          <CloseIcon className="h-4 w-4" />
        </button>
      )}
    </div>
  );
}