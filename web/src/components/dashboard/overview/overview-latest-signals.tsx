/**
 * Overview page — the latest signals block (server side).
 *
 * Self-contained async Server Component, same contract as
 * `OverviewTopPools`: fetches the seed (`GET /api/signals?limit=5`)
 * itself and degrades to a `BlockError` on failure so a signal hiccup
 * never takes down the rest of the page. The rendering and the live
 * SSE tail belong to the client child (`OverviewLatestSignalsLive`),
 * which reuses the exact `SignalCard` of the `/signals` feed.
 */

import { getTranslations } from "next-intl/server";

import { BlockError } from "@/components/dashboard/block-error";
import { LATEST_SIGNALS_COUNT } from "@/components/dashboard/signals/signal-display";
import { ApiClientError } from "@/lib/api/errors";
import type { SignalResponse } from "@/lib/api/schema/signal";
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
