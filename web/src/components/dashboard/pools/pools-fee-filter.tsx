/**
 * Fee-tier filter for the pools list.
 *
 * Client island on an otherwise Server-Component page. Its options are
 * the *most common* fee tiers actually observed in the data (fetched
 * server-side, passed in as `tiers`), each with its pool count — so the
 * filter can never offer a tier that would match nothing, and each row
 * shows how many pools it covers. Picking one drives the `fee_bps` URL
 * search param via `router.replace`; the "all fees" option clears it.
 *
 * A custom popover rather than a native `<select>`: an `<option>` can't
 * lay its content out (fee left, count right), so we mirror the house
 * `InfoPopover` pattern — a click-toggled button + a listbox panel,
 * closing on outside pointer-down or Escape. The options are real
 * `<button>`s so they stay keyboard-operable (Tab + Enter). Trade-off:
 * no native mobile picker, but for a short capped list a styled panel
 * reads fine.
 *
 * Changing the filter resets pagination: the `cursor` / `dir` /
 * `position` params are cleared, because a cursor into the previous
 * result set is meaningless once the filter changes — same rule as the
 * search box. `replace` rather than `push`: the filter is a refinement
 * of the current view, not a distinct destination worth a Back stop.
 *
 * The selection is seeded from the URL, so a shared link like
 * `/pools?fee_bps=25` pre-selects the tier on load.
 */

"use client";

import { useEffect, useId, useRef, useState } from "react";
import { useSearchParams } from "next/navigation";
import { useTranslations } from "next-intl";

import { ChevronDownIcon } from "@/components/shared/icon";
import { useRouter, usePathname } from "@/i18n/navigation";
import type { FeeTier } from "@/lib/api/server/fee-tiers";
import { formatFeeBps } from "@/lib/format/format-fee";

/** Sentinel value for "no fee filter". Empty string can't be a real
 *  tier, so it unambiguously means "all". */
const ALL = "";

export function PoolsFeeFilter({ tiers }: { tiers: FeeTier[] }) {
  const t = useTranslations("Dashboard.Pools.feeFilter");
  const router = useRouter();
  const pathname = usePathname();
  const searchParams = useSearchParams();

  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const listId = useId();

  // Seed from the URL so a shared/bookmarked ?fee_bps= pre-selects the
  // tier. An out-of-vocabulary value (a tier no longer among the most
  // common) simply matches no row, so the button falls back to "all".
  const current = searchParams.get("fee_bps") ?? ALL;
  const selectedLabel =
    current !== ALL ? formatFeeBps(current) : t("all");

  // Global listeners only while open: outside pointer-down or Escape
  // closes. Mirrors InfoPopover so the two feel identical.
  useEffect(() => {
    if (!open) {
      return;
    }
    const onPointerDown = (event: PointerEvent) => {
      if (rootRef.current && !rootRef.current.contains(event.target as Node)) {
        setOpen(false);
      }
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setOpen(false);
      }
    };
    document.addEventListener("pointerdown", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [open]);

  const select = (next: string) => {
    setOpen(false);

    const params = new URLSearchParams(searchParams.toString());
    if (next === ALL) {
      params.delete("fee_bps");
    } else {
      params.set("fee_bps", next);
    }

    // Reset pagination on any filter change.
    params.delete("cursor");
    params.delete("dir");
    params.delete("position");

    const qs = params.toString();
    router.replace(qs.length > 0 ? `${pathname}?${qs}` : pathname);
  };

  const rowBase =
    "flex w-full items-center justify-between gap-6 px-3 py-2 text-left text-sm transition-colors";

  return (
    <div ref={rootRef} className="relative w-full lg:w-[200px]">
      <button
        type="button"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={listId}
        aria-label={t("ariaLabel")}
        onClick={() => setOpen((value) => !value)}
        className="
          flex w-full items-center justify-between gap-2 rounded-md border
          border-sothoth-500/20 bg-cosmos-900/60 py-2 pr-2 pl-3 text-sm
          text-slate-100 transition-colors
          hover:border-sothoth-400/40
          focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sothoth-400
        "
      >
        <span className={current === ALL ? "text-slate-400" : undefined}>
          {selectedLabel}
        </span>
        <ChevronDownIcon
          className={`h-4 w-4 text-slate-500 transition-transform ${
            open ? "rotate-180" : ""
          }`}
        />
      </button>

      {open && (
        <ul
          id={listId}
          role="listbox"
          aria-label={t("ariaLabel")}
          className="
            absolute top-full right-0 left-0 z-20 mt-2 overflow-hidden rounded-md
            border border-sothoth-500/25 bg-cosmos-800
            shadow-[0_8px_24px_rgba(0,0,0,0.45)]
          "
        >
          <li role="option" aria-selected={current === ALL}>
            <button
              type="button"
              onClick={() => select(ALL)}
              className={`${rowBase} ${
                current === ALL
                  ? "bg-sothoth-500/10 text-slate-100"
                  : "text-slate-300 hover:bg-sothoth-500/[0.06]"
              }`}
            >
              <span>{t("all")}</span>
            </button>
          </li>

          {tiers.map((tier) => {
            const selected = tier.feeBps === current;
            return (
              <li key={tier.feeBps} role="option" aria-selected={selected}>
                <button
                  type="button"
                  onClick={() => select(tier.feeBps)}
                  className={`${rowBase} ${
                    selected
                      ? "bg-sothoth-500/10 text-slate-100"
                      : "text-slate-300 hover:bg-sothoth-500/[0.06]"
                  }`}
                >
                  <span>{formatFeeBps(tier.feeBps)}</span>
                  <span className="text-slate-500 tabular-nums">
                    {tier.poolCount}
                  </span>
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
