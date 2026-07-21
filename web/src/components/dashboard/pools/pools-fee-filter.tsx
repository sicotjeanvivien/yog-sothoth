/**
 * Fee-tier filter for the pools list.
 *
 * Client island on an otherwise Server-Component page. A native
 * `<select>` whose options are the fee tiers actually observed in the
 * data (fetched server-side, passed in as `tiers`) — so the filter can
 * never offer a tier that would match nothing. Picking one drives the
 * `fee_bps` URL search param via `router.replace`; the "all fees"
 * option clears it.
 *
 * Changing the filter resets pagination: the `cursor` / `dir` /
 * `position` params are cleared, because a cursor into the previous
 * result set is meaningless once the filter changes — same rule as the
 * search box.
 *
 * `replace` rather than `push`: the filter is a refinement of the
 * current view, not a distinct destination worth a Back-button stop.
 *
 * The selected value is seeded from the URL, so a shared link like
 * `/pools?fee_bps=25` pre-selects the tier on load.
 */

"use client";

import { useSearchParams } from "next/navigation";
import { useTranslations } from "next-intl";

import { ChevronDownIcon } from "@/components/shared/icon";
import { useRouter, usePathname } from "@/i18n/navigation";
import { formatFeeBps } from "@/lib/format/format-fee";

/** Sentinel `<option>` value for "no fee filter". Empty string can't be a
 *  real tier, so it unambiguously means "all". */
const ALL = "";

export function PoolsFeeFilter({ tiers }: { tiers: string[] }) {
  const t = useTranslations("Dashboard.Pools.feeFilter");
  const router = useRouter();
  const pathname = usePathname();
  const searchParams = useSearchParams();

  // Seed from the URL so a shared/bookmarked ?fee_bps= pre-selects the
  // tier. An out-of-vocabulary value (a tier no longer observed) simply
  // doesn't match any option, so the select falls back to "all".
  const current = searchParams.get("fee_bps") ?? ALL;

  const handleChange = (next: string) => {
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

  return (
    <div className="relative w-full lg:w-[180px]">
      <select
        value={current}
        onChange={(e) => handleChange(e.target.value)}
        aria-label={t("ariaLabel")}
        className="
          w-full appearance-none rounded-md border border-sothoth-500/20
          bg-cosmos-900/60 py-2 pl-3 pr-9 text-sm text-slate-100
          transition-colors
          focus:border-sothoth-400/50 focus:bg-cosmos-900/90
          focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sothoth-400
        "
      >
        <option value={ALL}>{t("all")}</option>
        {tiers.map((tier) => (
          <option key={tier} value={tier}>
            {formatFeeBps(tier)}
          </option>
        ))}
      </select>

      <span
        className="pointer-events-none absolute inset-y-0 right-2 flex items-center text-slate-500"
        aria-hidden="true"
      >
        <ChevronDownIcon className="h-4 w-4" />
      </span>
    </div>
  );
}
