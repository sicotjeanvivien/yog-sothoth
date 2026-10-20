"use client";

/**
 * The dashboard's navigation rail; `DashboardShell` owns its responsive state. ⚠️ `rail:sticky`
 * pins it only if no ancestor creates an `overflow` scroll context.
 */

import { useTranslations } from "next-intl";

import { Link } from "@/i18n/navigation";
import { LogoMark } from "@/components/shared/logo-mark";
import { ChevronLeftIcon, CloseIcon } from "@/components/shared/icon";

import { Preferences } from "./preferences";
import { SidebarNavList } from "./sidebar-nav-list";

type SidebarProps = {
  isOpen: boolean;
  /** Lets the shell close the drawer on navigation. */
  onNavigate: () => void;
  /** A narrow rail. ⚠️ The drawer ignores it: every `collapsed` style is `rail:`-scoped. */
  collapsed: boolean;
  onToggleCollapsed: () => void;
  /** Unfolds the rail, for the Preferences opened from it. */
  onExpand: () => void;
  initialPreferencesOpen: boolean;
};

export function Sidebar({
  isOpen,
  onNavigate,
  collapsed,
  onToggleCollapsed,
  onExpand,
  initialPreferencesOpen,
}: SidebarProps) {
  const t = useTranslations("Dashboard.shell");
  const positioning =
    "fixed top-0 left-0 z-40 h-dvh transition-[transform,width] duration-200 ease-out rail:sticky rail:z-auto rail:translate-x-0";
  const drawerState = isOpen ? "translate-x-0" : "-translate-x-full";
  const width = collapsed
    ? "w-[min(300px,85vw)] rail:w-16 rail:gap-5 rail:px-2 rail:py-5"
    : "w-[min(300px,85vw)] rail:w-[232px]";

  return (
    <aside
      id="dashboard-menu"
      className={`${positioning} ${drawerState} ${width} flex shrink-0 flex-col gap-7 border-r border-dash-rule bg-dash-bg px-5 py-6 font-dash-sans text-dash-ink`}
    >
      <button
        type="button"
        onClick={onNavigate}
        aria-label={t("closeMenu")}
        className="-mt-2 -mr-1.5 -mb-4 flex h-11 w-11 items-center justify-center self-end border border-dash-rule text-dash-ink rail:hidden"
      >
        <CloseIcon size={16} />
      </button>
      <div
        className={`flex items-center justify-between gap-2 ${collapsed ? "rail:flex-col rail:gap-3" : ""}`}
      >
        <Brand collapsed={collapsed} />
        <CollapseToggle collapsed={collapsed} onToggle={onToggleCollapsed} />
      </div>
      <nav className="min-h-0 flex-1 overflow-y-auto">
        <SidebarNavList collapsed={collapsed} onNavigate={onNavigate} />
      </nav>
      <Preferences
        initialOpen={initialPreferencesOpen}
        collapsed={collapsed}
        onExpand={onExpand}
      />
    </aside>
  );
}

/** The logo and wordmark, back to the site; `small` is the mobile bar's. */
export function Brand({
  collapsed = false,
  small = false,
}: {
  collapsed?: boolean;
  small?: boolean;
}) {
  const t = useTranslations("Brand");

  return (
    <div className={`flex ${collapsed ? "rail:justify-center" : ""}`}>
      <Link href="/" className="flex items-center gap-3">
        <LogoMark width={small ? 30 : 40} priority />
        <span
          className={`font-display text-[16px] font-semibold tracking-[0.16em] whitespace-nowrap uppercase ${collapsed ? "rail:sr-only" : ""}`}
        >
          {t("name")}
        </span>
      </Link>
    </div>
  );
}

/** The fixed rail only: a drawer has nothing to collapse. */
function CollapseToggle({
  collapsed,
  onToggle,
}: {
  collapsed: boolean;
  onToggle: () => void;
}) {
  const t = useTranslations("Dashboard.shell");
  const label = collapsed ? t("expandSidebar") : t("collapseSidebar");

  return (
    <button
      type="button"
      onClick={onToggle}
      aria-label={label}
      aria-expanded={!collapsed}
      aria-controls="dashboard-menu"
      title={label}
      className="hidden h-8 w-8 shrink-0 items-center justify-center border border-transparent text-dash-ink-3 transition-colors hover:border-dash-rule hover:text-dash-ink rail:inline-flex"
    >
      <ChevronLeftIcon size={14} className={collapsed ? "rotate-180" : ""} />
    </button>
  );
}
