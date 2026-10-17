"use client";

/**
 * The dashboard's navigation rail; `DashboardShell` owns its responsive state. ⚠️ `rail:sticky`
 * pins it only if no ancestor creates an `overflow` scroll context.
 */

import Image from "next/image";
import { useTranslations } from "next-intl";

import { Link } from "@/i18n/navigation";
import {
  ChevronDoubleLeftIcon,
  ChevronDoubleRightIcon,
  CloseIcon,
} from "@/components/shared/icon";

import { Preferences } from "./preferences";
import { SidebarNavList } from "./sidebar-nav-list";

type SidebarProps = {
  isOpen: boolean;
  /** Lets the shell close the drawer on navigation. */
  onNavigate: () => void;
  /** A narrow rail. ⚠️ The drawer ignores it: every `collapsed` style is `rail:`-scoped. */
  collapsed: boolean;
  onToggleCollapsed: () => void;
  initialPreferencesOpen: boolean;
};

export function Sidebar({
  isOpen,
  onNavigate,
  collapsed,
  onToggleCollapsed,
  initialPreferencesOpen,
}: SidebarProps) {
  const t = useTranslations("Dashboard.shell");
  const positioning =
    "fixed top-0 left-0 z-40 h-dvh transition-[transform,width] duration-200 ease-out rail:sticky rail:z-auto rail:translate-x-0";
  const drawerState = isOpen ? "translate-x-0" : "-translate-x-full";
  const width = collapsed
    ? "w-[min(300px,85vw)] rail:w-[76px] rail:px-3"
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
      <Brand collapsed={collapsed} />
      <nav className="min-h-0 flex-1 overflow-y-auto">
        <SidebarNavList collapsed={collapsed} onNavigate={onNavigate} />
      </nav>
      <div className="flex flex-col gap-3">
        <CollapseToggle collapsed={collapsed} onToggle={onToggleCollapsed} />
        <div className={collapsed ? "rail:hidden" : undefined}>
          <Preferences initialOpen={initialPreferencesOpen} />
        </div>
      </div>
    </aside>
  );
}

function Brand({ collapsed }: { collapsed: boolean }) {
  const t = useTranslations("Brand");

  return (
    <div className={`flex ${collapsed ? "rail:justify-center" : ""}`}>
      <Link href="/" className="flex items-center gap-3">
        <Image
          src="/logo_ys.png"
          alt=""
          width={48}
          height={54}
          priority
          className="h-13.5 w-12 shrink-0 object-contain"
        />
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
      title={collapsed ? label : undefined}
      className={`hidden p-1 text-dash-ink-3 transition-colors hover:text-dash-ink rail:flex ${collapsed ? "rail:self-center" : "rail:self-end"}`}
    >
      {collapsed ? (
        <ChevronDoubleRightIcon size={16} />
      ) : (
        <ChevronDoubleLeftIcon size={16} />
      )}
    </button>
  );
}
