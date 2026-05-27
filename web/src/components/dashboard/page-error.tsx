/**
 * Full-page error state.
 *
 * Used when the critical fetch of a dashboard page fails (e.g. the
 * pool record for /pools/[address] couldn't be loaded for a reason
 * other than 404, which is handled by Next.js' notFound()).
 *
 * The visual language is more prominent than `BlockError` — this
 * is the entire page's contents, not a substitution for a single
 * section. No retry button for the same reason as elsewhere; we
 * may add a small Client island later.
 */

import { getTranslations } from "next-intl/server";

import { AlertTriangleIcon } from "@/components/shared/icon";

import type { ApiClientErrorKind } from "@/lib/api/errors";
import { RetryButton } from "../shared/retry-button";

export async function PageError({ kind }: { kind: ApiClientErrorKind }) {
  const t = await getTranslations("Dashboard.PageError");

  return (
    <div className="mx-6 mt-12 lg:mx-10">
      <div className="flex flex-col items-center rounded-[8px] border border-amber-500/30 bg-amber-950/15 px-6 py-20 text-center">
        <div className="inline-flex h-[56px] w-[56px] items-center justify-center rounded-[6px] border border-amber-500/40 bg-amber-500/15 text-amber-400">
          <AlertTriangleIcon size={28} />
        </div>
        <h2 className="mt-6 font-display text-[22px] font-semibold tracking-[0.02em] text-amber-200">
          {t("title")}
        </h2>
        <p className="mt-4 max-w-[52ch] text-[14px] leading-[1.6] text-amber-100/80">
          {t(kind)}
        </p>
        <div className="py-4" >
          <RetryButton label={t("retry")} pendingLabel={t("retryPending")} />
        </div>
      </div>
    </div>
  );
}