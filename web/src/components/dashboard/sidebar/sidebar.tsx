"use client";

/**
 * Dashboard sidebar — the navigation rail: logo, menu, network status.
 *
 * Responsive state is NOT the sidebar's concern — `DashboardShell` owns
 * it and passes `isOpen`, `collapsed` and their callbacks.
 *
 * # Positioning — two modes at the lg breakpoint
 *
 *   >= lg : a permanent rail. `lg:sticky lg:top-0` keeps it pinned
 *           while the page scrolls — no ancestor may create an
 *           `overflow` scroll context, or sticky stops working.
 *   <  lg : an off-canvas drawer, `fixed` to the viewport and slid in
 *           when `isOpen`.
 *
 * Colours are the `dash-*` role tokens only; Cinzel is for the
 * wordmark alone.
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
  /** Whether the mobile drawer is open. Ignored on lg+. */
  isOpen: boolean;
  /** Called when a nav link is clicked — lets the shell close the drawer. */
  onNavigate: () => void;
  /**
   * lg+ only: a narrow rail instead of the full one. The mobile drawer
   * ignores it, so every `collapsed` style below is `lg:`-scoped.
   */
  collapsed: boolean;
  /** Toggle `collapsed` (the shell owns the state and its cookie). */
  onToggleCollapsed: () => void;
};

export function Sidebar({
  isOpen,
  onNavigate,
  collapsed,
  onToggleCollapsed,
}: SidebarProps) {
  const positioning =
    "fixed top-0 left-0 z-40 h-screen transition-[transform,width] duration-200 ease-out lg:sticky lg:z-auto lg:translate-x-0";
  const drawerState = isOpen ? "translate-x-0" : "-translate-x-full";
  const width = collapsed ? "w-[232px] lg:w-[76px] lg:px-3" : "w-[232px]";

  return (
    <aside
      className={`${positioning} ${drawerState} ${width} flex shrink-0 flex-col gap-7 border-r border-dash-rule bg-dash-bg px-5 py-6 font-dash-sans text-dash-ink`}
    >
      <BrandRow collapsed={collapsed} onToggle={onToggleCollapsed} />
      <nav className="flex-1">
        <SidebarNavList collapsed={collapsed} onNavigate={onNavigate} />
      </nav>
      <NetworkStatusPanel collapsed={collapsed} />
    </aside>
  );
}

/**
 * Logo and wordmark, linking to the site's home; on lg+ the collapse
 * toggle sits at the end of the row, under the logo once collapsed.
 */
function BrandRow({
  collapsed,
  onToggle,
}: {
  collapsed: boolean;
  onToggle: () => void;
}) {
  const t = useTranslations("Brand");
  const tShell = useTranslations("Dashboard.shell");
  const toggleLabel = collapsed
    ? tShell("expandSidebar")
    : tShell("collapseSidebar");

  return (
    <div
      className={`flex items-center justify-between gap-2 ${collapsed ? "lg:flex-col lg:gap-4" : ""}`}
    >
      <Link href="/" className="flex items-center gap-3">
        <Image
          src="/logo.png"
          alt={t("name")}
          width={40}
          height={43}
          priority
          className="h-auto w-10 shrink-0"
        />
        <span
          className={`font-display text-[16px] font-semibold tracking-[0.16em] uppercase ${collapsed ? "lg:hidden" : ""}`}
        >
          {t("name")}
        </span>
      </Link>
      <button
        type="button"
        onClick={onToggle}
        aria-label={toggleLabel}
        aria-expanded={!collapsed}
        title={collapsed ? toggleLabel : undefined}
        className="hidden p-1 text-dash-ink-3 transition-colors hover:text-dash-ink lg:flex"
      >
        {collapsed ? (
          <ChevronDoubleRightIcon size={16} />
        ) : (
          <ChevronDoubleLeftIcon size={16} />
        )}
      </button>
    </div>
  );
}
