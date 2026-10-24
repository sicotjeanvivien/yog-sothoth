/**
 * A protocol page's compact header: its platform, its title and an ⓘ with the program and since
 * when its figures are measured; at the right, "Live" over the period and the mode, as on the
 * Overview. ⚠️ The period links keep the tab, the tab links keep the period.
 */

import { Suspense } from "react";
import { getTranslations } from "next-intl/server";

import { LiveIndicator } from "@/components/dashboard/live-status/live-indicator";
import { AdvancedSwitch } from "@/components/dashboard/overview/advanced-switch";
import { LABEL_CLASS } from "@/components/dashboard/overview/overview-layout";
import {
  OverviewLiveRows,
  OverviewLiveRowsFallback,
} from "@/components/dashboard/overview/overview-live-rows";
import { utcMoment } from "@/components/dashboard/overview/overview-parts";
import { PeriodControl, PeriodSelect } from "@/components/dashboard/overview/period-control";
import { InfoPopover } from "@/components/shared/info-popover";
import type { PagePath } from "@/config/site";
import { ApiClientError } from "@/lib/api/errors";
import type { ProtocolResponse } from "@/lib/api/schema/zod.gen";
import { fetchProtocolActivity } from "@/lib/api/server/protocols";
import { shortenPubkey } from "@/lib/format/pubkey";
import type { Period } from "@/lib/overview/period";
import type { ProtocolTab } from "@/lib/protocol/tabs";

type HeaderProps = {
  pathname: PagePath;
  period: Period;
  tab: ProtocolTab | null;
  protocol: ProtocolResponse;
  programId: string;
};

export async function ProtocolHeader({ pathname, period, tab, protocol, programId }: HeaderProps) {
  const t = await getTranslations("Dashboard.Protocol.page");
  const tOverview = await getTranslations("Dashboard.Overview.page");
  const page = { pathname, keep: tab === null ? {} : { tab } };

  return (
    <header className="flex flex-wrap items-start justify-between gap-x-8 gap-y-3 border-b border-dash-rule px-4 py-5 font-dash-sans rail:px-8 rail:pt-4 rail:pb-[14px]">
      <div className="flex min-w-0 flex-[1_1_360px] flex-wrap items-baseline gap-x-2 gap-y-1.5">
        <span className={LABEL_CLASS}>{t("platform")}</span>
        <h1 className="text-[20px] leading-[1.2] font-semibold tracking-[-0.01em] text-dash-ink">
          <span aria-hidden="true" className="font-normal text-dash-ink-3">
            /{" "}
          </span>
          {t("title")}
        </h1>
        <span className="self-center">
          <InfoPopover label={t("infoLabel")} iconSize={14}>
            <span className="block font-dash-mono">
              {t("program", { id: shortenPubkey(programId, 6) })}
            </span>
            <Suspense fallback={null}>
              <MeasuredSince period={period} />
            </Suspense>
          </InfoPopover>
        </span>
      </div>
      <div className="flex flex-col items-start gap-2.5 rail:ml-auto rail:items-end">
        <span className="-ml-[10px] rail:-mr-[10px] rail:ml-0">
          <LiveIndicator
            pageRows={
              <Suspense key={period} fallback={<OverviewLiveRowsFallback />}>
                <OverviewLiveRows period={period} scope={protocol} />
              </Suspense>
            }
          />
        </span>
        <div className="flex flex-wrap items-center gap-3 rail:justify-end">
          <Suspense fallback={<PeriodSelect period={period} {...page} availability={null} />}>
            <PeriodControl period={period} {...page} />
          </Suspense>
          <AdvancedSwitch label={tOverview("advanced")} />
        </div>
      </div>
    </header>
  );
}

/** The first frozen hour, from which every figure of the page exists. */
async function MeasuredSince({ period }: { period: Period }) {
  const t = await getTranslations("Dashboard.Protocol.page");
  let since;
  try {
    since = (await fetchProtocolActivity(period)).coveredSince;
  } catch (err) {
    if (err instanceof ApiClientError) {
      return null;
    }
    throw err;
  }
  return (
    <span className="mt-1 block">
      {since === null ? t("measuredNone") : t("measuredSince", { since: await utcMoment(since) })}
    </span>
  );
}
