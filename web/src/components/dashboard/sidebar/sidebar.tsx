"use client";

/**
 * Dashboard sidebar — the navigation rail.
 *
 * Autonomous for routing concerns:
 *   - reads the current route via next-intl's `usePathname` (returns
 *     the path *without* the locale segment, e.g. `/overview`);
 *   - resolves its own labels via `useTranslations`;
 *   - reads the nav structure from `sidebar-nav.ts`.
 *
 * Responsive state, however, is NOT the sidebar's concern — it is
 * owned by `DashboardShell`, which passes `isOpen` and `onNavigate`.
 *
 * # Positioning — two modes at the lg breakpoint
 *
 *   >= lg : a permanent fixed rail. `lg:sticky lg:top-0` keeps it
 *           pinned while the page scrolls. For sticky to work no
 *           ancestor may create an `overflow` scroll context.
 *   <  lg : an off-canvas drawer. `fixed` to the viewport, slid out
 *           of view by default (`-translate-x-full`) and into view
 *           when `isOpen` (`translate-x-0`), with a transition.
 *
 * The mode is entirely CSS (Tailwind `lg:` variants); the component
 * only reads `isOpen` to pick the translate class below lg.
 *
 * # onNavigate
 *
 * Called whenever a nav link is clicked. The shell uses it to close
 * the drawer so navigation doesn't leave it open on the next page.
 * On lg+ it still fires but the shell's `close()` is a harmless no-op.
 *
 * # Visual identity
 *
 * Transposed from the validated HTML prototype:
 *   - brand block: logo PNG with a violet halo, Cinzel wordmark,
 *     spaced tagline;
 *   - nav items: near-rectangular (rounded-[3px]), icon + label;
 *   - active item: flat violet fill + a full-height accent bar on
 *     the left edge;
 *   - Solana Live footer panel pinned to the bottom.
 *
 * The Cinzel face is applied via the `font-display` utility, which
 * must be wired to a `next/font` instance in a root/locale layout
 * (see the integration notes delivered with this commit).
 */

import type { FC } from "react";
import Image from "next/image";
import { useTranslations } from "next-intl";

import { Link, usePathname } from "@/i18n/navigation";
import {
  ChevronDoubleLeftIcon,
  ChevronDoubleRightIcon,
  OverviewIcon,
  PoolsIcon,
  SignalsIcon,
  type IconProps,
} from "@/components/shared/icon";

import { SIDEBAR_NAV, type SidebarNavItem } from "./sidebar-nav";
import type { SidebarNavKey } from "./sidebar-keys";
import { NetworkStatusPanel } from "./network-status-panel";

// ── Icon mapping ──────────────────────────────────────────────────────
//
// Associates each nav key with its icon component. Lives here, in the
// sidebar, rather than in `sidebar-nav.ts` (which stays React-free
// pure data) or in `icon.tsx` (which stays a neutral icon library
// with no knowledge of the nav). The sidebar already maps keys to
// labels via i18n — mapping keys to icons is the same concern.
//
// The map is typed with `FC<IconProps>` — the exact type the icon
// components already expose — so the two sides agree without a
// narrower hand-rolled type that would reject `FC`'s return shape.

const NAV_ICONS: Record<SidebarNavKey, FC<IconProps>> = {
  overview: OverviewIcon,
  pools: PoolsIcon,
  signals: SignalsIcon,
};

// ── Active-item logic ─────────────────────────────────────────────────

/**
 * Whether a nav entry is the active one.
 *
 * Exact match only — `/pools` is active solely on `/pools`, not on
 * `/pools/<address>`. `usePathname` from next-intl already strips the
 * locale, so a plain equality is enough.
 */
function isItemActive(pathname: string, href: string): boolean {
  return pathname === href;
}

// ── Component ─────────────────────────────────────────────────────────

type SidebarProps = {
  /**
   * Whether the mobile drawer is open. Ignored on lg+ where the
   * sidebar is a permanent fixed rail.
   */
  isOpen: boolean;
  /** Called when a nav link is clicked — lets the shell close the drawer. */
  onNavigate: () => void;
  /**
   * lg+ only: icons-only rail (~76px) instead of the full 248px one.
   * The mobile drawer ignores it — collapsing an off-canvas drawer
   * makes no sense, so every `collapsed` style below is `lg:`-scoped.
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
  const pathname = usePathname();

  // Base: a fixed-position rail. Below lg it is an off-canvas drawer
  // translated in/out; on lg+ it switches to sticky and the translate
  // is neutralised (`lg:translate-x-0`).
  const positioning =
    "fixed top-0 left-0 z-40 h-screen transition-[transform,width] duration-200 ease-out lg:sticky lg:z-auto lg:translate-x-0";
  const drawerState = isOpen ? "translate-x-0" : "-translate-x-full";
  const width = collapsed ? "w-[248px] lg:w-[76px] lg:px-3" : "w-[248px]";

  return (
    <aside
      className={`${positioning} ${drawerState} ${width} flex shrink-0 flex-col border-r border-sothoth-700/25 bg-cosmos-900 px-5 pt-8 pb-6`}
    >
      <BrandBlock collapsed={collapsed} />
      <Divider />
      <nav className="flex flex-1 flex-col gap-[3px]">
        <NavHeader collapsed={collapsed} onToggle={onToggleCollapsed} />
        {SIDEBAR_NAV.map((item) => (
          <SidebarNavLink
            key={item.key}
            item={item}
            active={isItemActive(pathname, item.href)}
            onNavigate={onNavigate}
            collapsed={collapsed}
          />
        ))}
      </nav>
      <NetworkStatusPanel collapsed={collapsed} />
    </aside>
  );
}

// ── Brand ─────────────────────────────────────────────────────────────

/**
 * Brand block — logo, product name, tagline.
 * The logo lives at `web/public/logo.png` and is served from `/logo.png`.
 */
function BrandBlock({ collapsed }: { collapsed: boolean }) {
  const t = useTranslations("Brand");

  return (
    <Link href="/" className="flex items-center justify-center gap-[11px]">
      <div className="flex flex-col items-center px-1 pt-1 pb-2 text-center">
        <Image
          src="/logo.png"
          alt={t("name")}
          width={64}
          height={64}
          priority
          className={`object-contain [filter:drop-shadow(0_0_14px_rgba(139,92,246,0.55))] ${
            collapsed ? "h-[64] w-[64] lg:h-11 lg:w-11" : "h-[64] w-[64]"
          }`}
        />
        <p
          className={`mt-3 font-display text-[17px] font-semibold tracking-[0.22em] text-[#f1ecff] [text-indent:0.22em] [text-shadow:0_0_16px_rgba(139,92,246,0.75)] ${collapsed ? "lg:hidden" : ""}`}
        >
          {t("name")}
        </p>
        <p
          className={`mt-[7px] text-[10px] font-semibold tracking-[0.34em] text-sothoth-500 uppercase [text-indent:0.34em] ${collapsed ? "lg:hidden" : ""}`}
        >
          {t("tagline")}
        </p>
      </div>
    </Link>
  );
}

/** Horizontal rule with a violet glow fading out at both ends. */
function Divider() {
  return (
    <div className="mx-1 mt-[22px] mb-5 h-px bg-[linear-gradient(90deg,transparent,rgba(139,92,246,0.38)_20%,rgba(139,92,246,0.38)_80%,transparent)]" />
  );
}

// ── Navigation ────────────────────────────────────────────────────────

/**
 * Header row of the nav group: the small uppercase caption on the
 * left, the collapse/expand toggle on the right (lg+ only — an
 * off-canvas drawer has nothing to collapse). On the collapsed rail
 * the caption vanishes and the toggle takes the row, centered — the
 * first, most visible slot of the rail.
 */
function NavHeader({
  collapsed,
  onToggle,
}: {
  collapsed: boolean;
  onToggle: () => void;
}) {
  const t = useTranslations("Dashboard.Sidebar");
  const tShell = useTranslations("Dashboard.shell");
  const label = collapsed ? tShell("expandSidebar") : tShell("collapseSidebar");

  return (
    <div
      className={`mb-2 flex items-center justify-between px-[10px] ${collapsed ? "lg:justify-center lg:px-0" : ""}`}
    >
      <p
        className={`text-[10px] font-semibold tracking-[0.2em] text-slate-600 uppercase ${collapsed ? "lg:hidden" : ""}`}
      >
        {t("caption")}
      </p>
      <button
        type="button"
        onClick={onToggle}
        aria-label={label}
        aria-expanded={!collapsed}
        title={collapsed ? label : undefined}
        className="hidden rounded-[3px] p-1 text-slate-500 transition-colors hover:bg-sothoth-500/10 hover:text-slate-300 lg:flex"
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

/**
 * A single navigation link. Purely presentational — it is told
 * whether it is active rather than working it out itself.
 *
 * The active state combines a flat violet fill and a full-height
 * accent bar (`before:`) clipped to the rounded corners by
 * `overflow-hidden`.
 *
 * `onNavigate` fires on click so the shell can close the mobile
 * drawer. On lg+ it is a harmless no-op.
 */
function SidebarNavLink({
  item,
  active,
  onNavigate,
  collapsed,
}: {
  item: SidebarNavItem;
  active: boolean;
  onNavigate: () => void;
  collapsed: boolean;
}) {
  const t = useTranslations("Dashboard.Sidebar.nav");

  const base = `relative flex items-center gap-3 overflow-hidden rounded-[3px] px-3 py-[11px] text-[14px] font-medium transition-colors ${
    collapsed ? "lg:justify-center" : ""
  }`;

  const state = active
    ? "bg-sothoth-600/20 text-[#f1ecff] before:absolute before:inset-y-0 before:left-0 before:w-[3px] before:bg-sothoth-500 before:shadow-[0_0_12px_1px_rgba(139,92,246,0.85)]"
    : "text-slate-400 hover:bg-sothoth-500/10 hover:text-slate-300";

  const Icon = NAV_ICONS[item.key];

  return (
    <Link
      href={item.href}
      onClick={onNavigate}
      className={`${base} ${state}`}
      aria-current={active ? "page" : undefined}
      // Native browser tooltip on the collapsed rail — a custom
      // floating panel would overlap the page content; the OS-managed
      // one never fights the layout.
      title={collapsed ? t(item.labelKey) : undefined}
    >
      <Icon size={18} />
      <span className={`leading-none ${collapsed ? "lg:hidden" : ""}`}>
        {t(item.labelKey)}
      </span>
    </Link>
  );
}


