/**
 * The tab 06 of a protocol's page: the signals that arrive while it is open, and in the advanced
 * mode its live activity beside them, on one connection to the protocol's live stream.
 * ⚠️ No signal is loaded before the stream: the block starts empty, like the activity table.
 */

import { getTranslations } from "next-intl/server";

import { ActivityBlock } from "@/components/dashboard/activity/activity-block";
import { OverviewLive } from "@/components/dashboard/live/overview-live";
import { CELL_CLASS, GRID_CLASS, MIN_HEIGHT } from "@/components/dashboard/overview/overview-layout";
import { BlockHeading } from "@/components/dashboard/overview/overview-parts";
import { OverviewSignalsLive } from "@/components/dashboard/overview/overview-signals-live";
import type { ProtocolResponse } from "@/lib/api/schema/zod.gen";

/** Beside the live activity in the advanced mode, so the tab is no taller there. */
const SPAN = "col-span-12 adv:dash-wide:col-span-6";

export async function ActivityTab({ protocol }: { protocol: ProtocolResponse }) {
  const t = await getTranslations("Dashboard.Protocol.activity.signals");

  return (
    <OverviewLive protocol={protocol}>
    <div className={GRID_CLASS}>
      <ActivityBlock
        scope="protocol"
        span="col-span-12 dash-wide:col-span-6"
        minHeight={MIN_HEIGHT.activity}
      />
      <section aria-label={t("title")} className={`${CELL_CLASS} ${SPAN} ${MIN_HEIGHT.signals}`}>
        <OverviewSignalsLive
          initial={[]}
          heading={<BlockHeading title={t("title")} info={t("info")} />}
          tableLabel={t("tableLabel")}
        />
      </section>
    </div>
    </OverviewLive>
  );
}
