/**
 * The Overview's signals: fetches the seed and degrades to a `BlockError` on its own; the
 * rows and the live tail belong to `OverviewSignalsLive`.
 */

import { getTranslations } from "next-intl/server";

import { BlockError } from "@/components/dashboard/block-error";
import { LATEST_SIGNALS_COUNT } from "@/components/dashboard/signals/signal-display";
import { ApiClientError } from "@/lib/api/errors";
import type { SignalResponse } from "@/lib/api/schema/zod.gen";
import { fetchSignals } from "@/lib/api/server/signals";

import { BlockPlaceholder } from "../block-placeholder";
import { BlockHeading } from "./overview-parts";
import { CELL_CLASS, MIN_HEIGHT, SPAN } from "./overview-layout";
import { OverviewSignalsLive } from "./overview-signals-live";

const SECTION_CLASS = `${CELL_CLASS} ${SPAN.side} ${MIN_HEIGHT.signals}`;

export async function OverviewSignals() {
  const t = await getTranslations("Dashboard.Overview.signals");

  let initial: readonly SignalResponse[];
  try {
    initial = (await fetchSignals({ limit: LATEST_SIGNALS_COUNT })).items;
  } catch (err) {
    if (err instanceof ApiClientError) {
      return (
        <section className={SECTION_CLASS}>
          <BlockError title={t("title")} kind={err.details.kind} />
        </section>
      );
    }
    throw err;
  }

  return (
    <section aria-label={t("title")} className={SECTION_CLASS}>
      <OverviewSignalsLive
        initial={initial}
        heading={<BlockHeading title={t("title")} info={t("info")} />}
      />
    </section>
  );
}

export async function OverviewSignalsPlaceholder() {
  return <BlockPlaceholder className={`${SPAN.side} ${MIN_HEIGHT.signals}`} />;
}
