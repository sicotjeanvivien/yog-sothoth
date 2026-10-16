/**
 * The error state of one block whose secondary fetch failed, in place of its table or chart.
 * No retry button: the page is a Server Component, and a refresh retries.
 */

import { getTranslations } from "next-intl/server";

import { AlertTriangleIcon } from "@/components/shared/icon";

import type { ApiClientErrorKind } from "@/lib/api/errors";
import { RetryButton } from "../shared/retry-button";

const CARD_CLASS =
  "rounded-[8px] border border-amber-500/30 bg-amber-950/15";

const TITLE_BAR_CLASS =
  "flex items-center justify-between border-b border-amber-500/20 px-6 py-4";

const SECTION_TITLE_CLASS =
  "text-[12px] font-semibold tracking-[0.2em] text-amber-200/80 uppercase";

const BODY_CLASS = "flex flex-col items-center px-6 py-12 text-center";

const ICON_WRAP_CLASS =
  "inline-flex h-[40px] w-[40px] items-center justify-center rounded-[6px] border border-amber-500/40 bg-amber-500/15 text-amber-400";

const MESSAGE_CLASS =
  "mt-4 max-w-[52ch] text-[14px] leading-[1.6] text-amber-100/80";

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
        <div className="py-4">
          <RetryButton label={t("retry")} pendingLabel={t("retryPending")} variant="block" />
        </div>
      </div>
    </div>
  );
}