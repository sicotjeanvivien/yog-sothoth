/** No pool at all: most likely nothing captured yet, and the message says so. */

import { getTranslations } from "next-intl/server";

import { PoolsIcon } from "@/components/shared/icon";

export async function PoolsEmpty() {
  const t = await getTranslations("Dashboard.Pools.empty");

  return (
    <div className="mx-6 lg:mx-10">
      <div className="flex flex-col items-center rounded-[8px] border border-dashed border-sothoth-500/20 bg-cosmos-900/40 px-6 py-16 text-center">
        <div className="inline-flex h-[48px] w-[48px] items-center justify-center rounded-[6px] border border-sothoth-500/25 bg-sothoth-600/10 text-sothoth-400">
          <PoolsIcon size={24} />
        </div>
        <h2 className="mt-5 font-display text-[18px] font-semibold tracking-[0.02em] text-[#f1ecff]">
          {t("title")}
        </h2>
        <p className="mt-3 max-w-[52ch] text-[14px] leading-[1.6] text-slate-400">
          {t("description")}
        </p>
      </div>
    </div>
  );
}