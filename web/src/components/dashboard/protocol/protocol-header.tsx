/**
 * A protocol page's compact header: its platform, its title and an ⓘ with the program and since
 * when its figures are measured; at the right, the Overview's controls. ⚠️ The period links
 * keep the tab, the tab links keep the period.
 */

import { Suspense } from "react";
import { getTranslations } from "next-intl/server";

import { HeaderControls } from "@/components/dashboard/overview/header-controls";
import { LABEL_CLASS } from "@/components/dashboard/overview/overview-layout";
import { utcMoment } from "@/components/dashboard/overview/overview-parts";
import { InfoPopover } from "@/components/shared/info-popover";
import type { PagePath } from "@/config/site";
import { ApiClientError } from "@/lib/api/errors";
import type { ProtocolResponse } from "@/lib/api/schema/zod.gen";
import { fetchSwapTotals } from "@/lib/api/server/protocols";
import { shortenPubkey } from "@/lib/format/pubkey";
import { DEFAULT_PERIOD, OVERVIEW_PERIODS, type Period } from "@/lib/overview/period";
import { tabQuery, type ProtocolTab } from "@/lib/protocol/tabs";

type HeaderProps = {
  pathname: PagePath;
  period: Period;
  tab: ProtocolTab | null;
  protocol: ProtocolResponse;
  programId: string;
};

export async function ProtocolHeader({ pathname, period, tab, protocol, programId }: HeaderProps) {
  const t = await getTranslations("Dashboard.Protocol.page");

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
              <MeasuredSince protocol={protocol} />
            </Suspense>
          </InfoPopover>
        </span>
      </div>
      <HeaderControls
        period={period}
        periods={OVERVIEW_PERIODS}
        scope={protocol}
        pathname={pathname}
        keep={tabQuery(tab)}
      />
    </header>
  );
}

/** The first frozen hour, from which every figure of the page exists. */
async function MeasuredSince({ protocol }: { protocol: ProtocolResponse }) {
  const t = await getTranslations("Dashboard.Protocol.page");
  let since;
  try {
    // ⚠️ Not the page's period: over the last hour, the coverage is the tranches still held.
    since = (await fetchSwapTotals(protocol, DEFAULT_PERIOD)).coveredSince;
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
