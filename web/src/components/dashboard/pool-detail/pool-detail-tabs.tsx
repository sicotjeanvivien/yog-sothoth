/**
 * The pool detail's tabs, from `?tab=`, as `<Link>`s of a Server Component: the page fetches
 * only the active tab. A tab link drops the other tabs' pagination params.
 */

import { getTranslations } from "next-intl/server";

import { Link } from "@/i18n/navigation";

export const TAB_IDS = ["info", "swaps", "liquidity", "fees", "alerts"] as const;
export type TabId = (typeof TAB_IDS)[number];

const DEFAULT_TAB: TabId = "info";

/** A known tab id, `info` by default. */
export function parseTab(raw: string | string[] | undefined): TabId {
  return TAB_IDS.includes(raw as TabId) ? (raw as TabId) : DEFAULT_TAB;
}

const NAV_CLASS =
  "flex items-center gap-1 border-b border-sothoth-500/20 px-6 lg:px-10";

const TAB_BASE =
  "relative px-4 py-3 text-[14px] font-medium tracking-wide transition-colors " +
  "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sothoth-400 rounded-t-md";

const TAB_ACTIVE = "text-slate-100";
const TAB_INACTIVE = "text-slate-400 hover:text-slate-200";

const ACTIVE_UNDERLINE =
  "absolute inset-x-3 -bottom-px h-0.5 rounded-full bg-sothoth-400";

export async function PoolDetailTabs({
  basePath,
  activeTab,
}: {
  basePath: string;
  activeTab: TabId;
}) {
  const t = await getTranslations("Dashboard.PoolDetail.tabs");

  return (
    <nav className={`mt-6 ${NAV_CLASS}`} aria-label={t("ariaLabel")}>
      {TAB_IDS.map((id) => {
        const isActive = id === activeTab;
        return (
          <Link
            key={id}
            href={`${basePath}?tab=${id}`}
            aria-current={isActive ? "page" : undefined}
            className={`${TAB_BASE} ${isActive ? TAB_ACTIVE : TAB_INACTIVE}`}
          >
            {t(id)}
            {isActive && <span className={ACTIVE_UNDERLINE} />}
          </Link>
        );
      })}
    </nav>
  );
}
