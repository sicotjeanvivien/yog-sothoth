/**
 * Search box for the pools list.
 *
 * Client island on an otherwise Server-Component page. Debounced
 * (300ms) input that drives the `q` URL search param via
 * `router.replace`. Typing a new query resets pagination: the
 * `cursor` / `dir` / `position` params are cleared, because a cursor
 * into the previous (unfiltered or differently-filtered) result set
 * is meaningless once the filter changes.
 *
 * `replace` rather than `push`: a search-as-you-type box would
 * otherwise stack one history entry per debounced keystroke, making
 * the browser Back button useless.
 *
 * The initial value comes from the URL so a shared link like
 * `/pools?q=SOL` pre-fills the box on load.
 */

"use client";

import { useEffect, useRef, useState } from "react";
import { useSearchParams } from "next/navigation";
import { useTranslations } from "next-intl";

import { useRouter, usePathname } from "@/i18n/navigation";
import { SearchIcon, CloseIcon } from "@/components/shared/icon"; // adapt if names differ

const DEBOUNCE_MS = 300;

export function PoolsSearch() {
  const t = useTranslations("Dashboard.Pools.search");
  const router = useRouter();
  const pathname = usePathname();
  const searchParams = useSearchParams();

  // Seed from the URL so a shared/bookmarked ?q= pre-fills the input.
  const initialQuery = searchParams.get("q") ?? "";
  const [value, setValue] = useState(initialQuery);

  // Keep the input in sync if the URL changes from the outside
  // (e.g. browser Back/Forward landing on a different ?q=). We only
  // overwrite when the incoming URL value actually differs from what
  // we last pushed, to avoid clobbering mid-typing.
  const lastPushedRef = useRef(initialQuery);
  useEffect(() => {
    const urlQuery = searchParams.get("q") ?? "";
    if (urlQuery !== lastPushedRef.current) {
      setValue(urlQuery);
      lastPushedRef.current = urlQuery;
    }
  }, [searchParams]);

  // Debounced navigation. Whenever `value` settles for DEBOUNCE_MS,
  // push it to the URL (or clear it). Pagination params are dropped.
  useEffect(() => {
    const handle = setTimeout(() => {
      const trimmed = value.trim();
      const current = searchParams.get("q") ?? "";

      // No-op if nothing changed — avoids a redundant navigation
      // (and an infinite loop with the sync effect above).
      if (trimmed === current) return;

      const params = new URLSearchParams(searchParams.toString());

      if (trimmed.length > 0) {
        params.set("q", trimmed);
      } else {
        params.delete("q");
      }

      // Reset pagination on any query change.
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
    // The debounce effect will fire and clear `q` from the URL.
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
        maxLength={100}
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