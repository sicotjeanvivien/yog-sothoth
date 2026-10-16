/** The pool detail's 404: a well-formed address the indexer has never observed. */

import { getTranslations } from "next-intl/server";

import { Link } from "@/i18n/navigation";

import { ArrowLeftIcon } from "@/components/shared/icon";

export default async function PoolNotFound() {
  const t = await getTranslations("Dashboard.PoolDetail.notFound");

  return (
    <div className="mx-6 mt-12 lg:mx-10">
      <div className="flex flex-col items-center rounded-[8px] border border-sothoth-500/15 bg-cosmos-900/40 px-6 py-20 text-center">
        <h1 className="font-display text-[24px] font-semibold tracking-[0.02em] text-[#f5f2ff]">
          {t("title")}
        </h1>
        <p className="mt-4 max-w-[52ch] text-[14px] leading-[1.6] text-slate-400">
          {t("description")}
        </p>
        <Link
          href="/pools"
          className="mt-8 inline-flex items-center gap-2 rounded-[4px] border border-sothoth-500/45 bg-sothoth-600/15 px-5 py-[10px] text-[14px] font-semibold text-[#f1ecff] transition-colors hover:border-sothoth-500/70 hover:bg-sothoth-600/30"
        >
          <ArrowLeftIcon size={14} />
          {t("backToList")}
        </Link>
      </div>
    </div>
  );
}