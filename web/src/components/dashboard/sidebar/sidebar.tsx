"use client";

/**
 * The dashboard's navigation rail; `DashboardShell` owns its responsive state. ⚠️ `lg:sticky`
 * pins it only if no ancestor creates an `overflow` scroll context.
 */

import Image from "next/image";
import { useTranslations } from "next-intl";

import { Link } from "@/i18n/navigation";
import {
  ChevronDoubleLeftIcon,
  ChevronDoubleRightIcon,
} from "@/components/shared/icon";

import { NetworkStatusPanel } from "./network-status-panel";
import { SidebarNavList } from "./sidebar-nav-list";

type SidebarProps = {
  isOpen: boolean;
  /** Lets the shell close the drawer on navigation. */
  onNavigate: () => void;
  /** A narrow rail. ⚠️ The drawer ignores it: every `collapsed` style is `lg:`-scoped. */
  collapsed: boolean;
  onToggleCollapsed: () => void;
};

export function Sidebar({
  isOpen,
  onNavigate,
  collapsed,
  onToggleCollapsed,
}: SidebarProps) {
  const positioning =
    "fixed top-0 left-0 z-40 h-dvh transition-[transform,width] duration-200 ease-out lg:sticky lg:z-auto lg:translate-x-0";
  const drawerState = isOpen ? "translate-x-0" : "-translate-x-full";
  const width = collapsed ? "w-[232px] lg:w-[76px] lg:px-3" : "w-[232px]";

  return (
    <aside
      className={`${positioning} ${drawerState} ${width} flex shrink-0 flex-col gap-7 border-r border-dash-rule bg-dash-bg px-5 py-6 font-dash-sans text-dash-ink`}
    >
      <Brand collapsed={collapsed} />
      <nav className="min-h-0 flex-1 overflow-y-auto">
        <SidebarNavList collapsed={collapsed} onNavigate={onNavigate} />
      </nav>
      <div className="flex flex-col gap-3">
        <CollapseToggle collapsed={collapsed} onToggle={onToggleCollapsed} />
        <NetworkStatusPanel collapsed={collapsed} />
      </div>
    </aside>
  );
}

function Brand({ collapsed }: { collapsed: boolean }) {
  const t = useTranslations("Brand");

  return (
    <div className={`flex ${collapsed ? "lg:justify-center" : ""}`}>
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
          className={`font-display text-[16px] font-semibold tracking-[0.16em] whitespace-nowrap uppercase ${collapsed ? "lg:sr-only" : ""}`}
        >
          {t("name")}
        </span>
      </Link>
    </div>
  );
}

/** `lg` and up only: a drawer has nothing to collapse. */
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
      className={`hidden p-1 text-dash-ink-3 transition-colors hover:text-dash-ink lg:flex ${collapsed ? "lg:self-center" : "lg:self-end"}`}
    >
      {collapsed ? (
        <ChevronDoubleRightIcon size={16} />
      ) : (
        <ChevronDoubleLeftIcon size={16} />
      )}
    </button>
  );
}
