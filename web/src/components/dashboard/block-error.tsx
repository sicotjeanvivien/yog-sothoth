/**
 * Generic per-block error state.
 *
 * Used when one of the secondary fetches on a page fails — the
 * page itself is still functional (the critical data loaded), but
 * one section couldn't be assembled. The component sits in place
 * of the table, list, or chart that would normally render there.
 *
 * Receives:
 *   - `title`: localised section label (e.g. "Recent swaps") so
 *     the visitor knows which block failed
 *   - `kind`: the `ApiClientErrorKind` discriminator, used to pick
 *     the right localised message
 *
 * No retry button — same rationale as `PoolsError` on /pools: the
 * page is a Server Component, retry means a navigation refresh
 * which the visitor can trigger themselves. We may revisit this
 * later with a tiny Client island.
 */

import { getTranslations } from "next-intl/server";

import { AlertTriangleIcon } from "@/components/shared/icon";

import type { ApiClientErrorKind } from "@/lib/api/errors";

const CARD_CLASS =
  "rounded-[8px] border border-amber-500/30 bg-amber-950/15";

const TITLE_BAR_CLASS =
  "flex items-center justify-between border-b border-amber-500/20 px-6 py-4";

const SECTION_TITLE_CLASS =
  "text-[11px] font-semibold tracking-[0.2em] text-amber-200/80 uppercase";

const BODY_CLASS = "flex flex-col items-center px-6 py-12 text-center";

const ICON_WRAP_CLASS =
  "inline-flex h-[40px] w-[40px] items-center justify-center rounded-[6px] border border-amber-500/40 bg-amber-500/15 text-amber-400";

const MESSAGE_CLASS =
  "mt-4 max-w-[52ch] text-[13px] leading-[1.6] text-amber-100/80";

export async function BlockError({
  title,
  kind,
}: {
  title: string;
  kind: ApiClientErrorKind;
}) {
  const t = await getTranslations("Dashboard.BlockError");

  return (
    <div className={CARD_CLASS}>
      <div className={TITLE_BAR_CLASS}>
        <h2 className={SECTION_TITLE_CLASS}>{title}</h2>
      </div>
      <div className={BODY_CLASS}>
        <div className={ICON_WRAP_CLASS}>
          <AlertTriangleIcon size={20} />
        </div>
        <p className={MESSAGE_CLASS}>{t(kind)}</p>
      </div>
    </div>
  );
}