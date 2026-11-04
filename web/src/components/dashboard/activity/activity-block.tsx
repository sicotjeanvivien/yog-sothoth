/**
 * The live activity block, shown in the advanced mode only: its heading on the server, its rows
 * in the `LiveActivity` client. Nothing to fetch first, so it has no waiting slot.
 */

import { getTranslations } from "next-intl/server";

import { ADVANCED_ONLY, CELL_CLASS } from "@/components/dashboard/overview/overview-layout";
import { BlockHeading } from "@/components/dashboard/overview/overview-parts";

import { LiveActivity } from "./live-activity";

export async function ActivityBlock({
  scope,
  span,
  minHeight,
}: {
  /** Every protocol's activity, or the protocol's of the page. */
  scope: "all" | "protocol";
  span: string;
  minHeight: string;
}) {
  const t = await getTranslations("Dashboard.Activity");
  const title = scope === "all" ? t("title") : t("titleProtocol");

  return (
    <section
      aria-label={title}
      className={`${CELL_CLASS} ${span} ${minHeight} ${ADVANCED_ONLY.block}`}
    >
      <LiveActivity
        heading={<BlockHeading title={title} info={t("info")} />}
        tableLabel={t("tableLabel")}
        withProtocol={scope === "all"}
      />
    </section>
  );
}
