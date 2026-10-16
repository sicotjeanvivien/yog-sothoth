/**
 * The Overview's latest signals: fetches the seed and degrades to a `BlockError` on its own;
 * rendering and the live tail belong to `OverviewLatestSignalsLive`.
 */

import { getTranslations } from "next-intl/server";

import { BlockError } from "@/components/dashboard/block-error";
import { LATEST_SIGNALS_COUNT } from "@/components/dashboard/signals/signal-display";
import { ApiClientError } from "@/lib/api/errors";
import type { SignalResponse } from "@/lib/api/schema/zod.gen";
import { fetchSignals } from "@/lib/api/server/signals";

import { OverviewLatestSignalsLive } from "./overview-latest-signals-live";

export async function OverviewLatestSignals() {
  const t = await getTranslations("Dashboard.Overview.latestSignals");

  let initial: readonly SignalResponse[];
  try {
    initial = (await fetchSignals({ limit: LATEST_SIGNALS_COUNT })).items;
  } catch (err) {
    if (err instanceof ApiClientError) {
      return <BlockError title={t("title")} kind={err.details.kind} />;
    }
    throw err;
  }

  return <OverviewLatestSignalsLive initial={initial} />;
}
