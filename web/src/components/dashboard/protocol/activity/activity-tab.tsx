/**
 * The tab 06 of a protocol's page: its latest signals whatever the period, then those that
 * arrive, on the protocol's live stream. The seed fails on its own, as a `BlockError`.
 */

import { Suspense } from "react";
import { getTranslations } from "next-intl/server";

import { CELL_CLASS, GRID_CLASS, MIN_HEIGHT } from "@/components/dashboard/overview/overview-layout";
import { BlockHeading } from "@/components/dashboard/overview/overview-parts";
import { OverviewSignalsLive } from "@/components/dashboard/overview/overview-signals-live";
import { BlockFailed, BlockSkeleton } from "@/components/dashboard/overview/overview-states";
import { LATEST_SIGNALS_COUNT } from "@/components/dashboard/signals/signal-display";
import { ApiClientError } from "@/lib/api/errors";
import type { ProtocolResponse, SignalResponse } from "@/lib/api/schema/zod.gen";
import { fetchSignals } from "@/lib/api/server/signals";

const SPAN = "col-span-12";

export async function ActivityTab({ protocol }: { protocol: ProtocolResponse }) {
  const t = await getTranslations("Dashboard.Protocol.activity.signals");

  return (
    <div className={GRID_CLASS}>
      <Suspense
        fallback={
          <BlockSkeleton
            title={t("title")}
            span={SPAN}
            minHeight={MIN_HEIGHT.signals}
            shape={{ rows: 7 }}
          />
        }
      >
        <ProtocolSignals protocol={protocol} />
      </Suspense>
    </div>
  );
}

async function ProtocolSignals({ protocol }: { protocol: ProtocolResponse }) {
  const t = await getTranslations("Dashboard.Protocol.activity.signals");
  const className = `${CELL_CLASS} ${SPAN} ${MIN_HEIGHT.signals}`;
  const heading = <BlockHeading title={t("title")} info={t("info")} />;

  let initial: readonly SignalResponse[];
  try {
    initial = (await fetchSignals({ limit: LATEST_SIGNALS_COUNT, protocol })).items;
  } catch (err) {
    if (err instanceof ApiClientError) {
      return (
        <section aria-label={t("title")} className={className}>
          {heading}
          <BlockFailed kind={err.details.kind} />
        </section>
      );
    }
    throw err;
  }

  return (
    <section aria-label={t("title")} className={className}>
      <OverviewSignalsLive
        initial={initial}
        heading={heading}
        tableLabel={t("tableLabel")}
        protocol={protocol}
      />
    </section>
  );
}
