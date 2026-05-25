/**
 * Error state for the pools table.
 *
 * Receives the `kind` of the underlying `ApiClientError` and
 * shows the appropriate localised message. The four kinds align
 * with `ApiClientErrorKind` from `lib/api/errors`.
 *
 * No retry button in this commit — the page is a Server Component,
 * retry would need a client island that triggers a navigation
 * refresh; deferred to a later change.
 */

import { getTranslations } from "next-intl/server";

import { AlertTriangleIcon } from "@/components/shared/icon";

import type { ApiClientErrorKind } from "@/lib/api/errors";

export async function PoolsError({ kind }: { kind: ApiClientErrorKind }) {
  const t = await getTranslations("Dashboard.Pools.error");

  return (
    <div className="mx-6 lg:mx-10">
      <div className="flex flex-col items-center rounded-[8px] border border-amber-500/30 bg-amber-950/15 px-6 py-16 text-center">
        <div className="inline-flex h-[48px] w-[48px] items-center justify-center rounded-[6px] border border-amber-500/40 bg-amber-500/15 text-amber-400">
          <AlertTriangleIcon size={24} />
        </div>
        <h2 className="mt-5 font-display text-[18px] font-semibold tracking-[0.02em] text-amber-200">
          {t("title")}
        </h2>
        <p className="mt-3 max-w-[52ch] text-[14px] leading-[1.6] text-amber-100/80">
          {t(kind)}
        </p>
      </div>
    </div>
  );
}