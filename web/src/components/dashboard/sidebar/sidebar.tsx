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
  OverviewIcon,
  PoolsIcon,
  SolanaGlyph,
  type IconProps,
} from "@/components/shared/icon";

import { SIDEBAR_NAV, type SidebarNavItem } from "./sidebar-nav";
import type { SidebarNavKey } from "./sidebar-keys";

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
};

export function Sidebar({ isOpen, onNavigate }: SidebarProps) {
  const pathname = usePathname();

  // Base: a fixed-position rail. Below lg it is an off-canvas drawer
  // translated in/out; on lg+ it switches to sticky and the translate
  // is neutralised (`lg:translate-x-0`).
  const positioning =
    "fixed top-0 left-0 z-40 h-screen transition-transform duration-200 ease-out lg:sticky lg:z-auto lg:translate-x-0";
  const drawerState = isOpen ? "translate-x-0" : "-translate-x-full";

  return (
    <aside
      className={`${positioning} ${drawerState} flex w-[248px] shrink-0 flex-col border-r border-sothoth-700/25 bg-cosmos-900 px-5 pt-8 pb-6`}
    >
      <BrandBlock />
      <Divider />
      <nav className="flex flex-1 flex-col gap-[3px]">
        <NavCaption />
        {SIDEBAR_NAV.map((item) => (
          <SidebarNavLink
            key={item.key}
            item={item}
            active={isItemActive(pathname, item.href)}
            onNavigate={onNavigate}
          />
        ))}
      </nav>
      <SolanaLivePanel />
    </aside>
  );
}

// ── Brand ─────────────────────────────────────────────────────────────

/**
 * Brand block — logo, product name, tagline.
 * The logo lives at `web/public/logo.png` and is served from `/logo.png`.
 */
function BrandBlock() {
  const t = useTranslations("Brand");

  return (
    <Link href="/" className="flex items-center gap-[11px]">
      <div className="flex flex-col items-center px-1 pt-1 pb-2 text-center">
        <Image
          src="/logo.png"
          alt={t("name")}
          width={84}
          height={84}
          priority
          className="h-[auto] w-[auto] object-contain [filter:drop-shadow(0_0_14px_rgba(139,92,246,0.55))]"
        />
        <p className="mt-3 font-display text-[17px] font-semibold tracking-[0.22em] text-[#f1ecff] [text-indent:0.22em] [text-shadow:0_0_16px_rgba(139,92,246,0.75)]">
          {t("name")}
        </p>
        <p className="mt-[7px] text-[9px] font-semibold tracking-[0.34em] text-sothoth-500 uppercase [text-indent:0.34em]">
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

/** Small uppercase caption above the nav group. */
function NavCaption() {
  const t = useTranslations("Dashboard.Sidebar");
  return (
    <p className="mb-2 px-[10px] text-[9px] font-semibold tracking-[0.2em] text-slate-600 uppercase">
      {t("caption")}
    </p>
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
}: {
  item: SidebarNavItem;
  active: boolean;
  onNavigate: () => void;
}) {
  const t = useTranslations("Dashboard.Sidebar.nav");

  const base =
    "relative flex items-center gap-3 overflow-hidden rounded-[3px] px-3 py-[11px] text-[13.5px] font-medium transition-colors";

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
    >
      <Icon size={18} />
      <span className="leading-none">{t(item.labelKey)}</span>
    </Link>
  );
}

// ── Solana Live footer ────────────────────────────────────────────────

/**
 * Network status panel pinned to the bottom of the rail.
 *
 * Slot and latency are placeholders in this commit — the panel shows
 * its final visual form, but the values are static. Live data will be
 * wired in a follow-up via a dedicated network-status route.
 */
function SolanaLivePanel() {
  const t = useTranslations("Dashboard.Sidebar.network");

  return (
    <div className="mt-5 rounded-[4px] border border-sothoth-500/15 bg-cosmos-800/65 p-[14px]">
      <header className="flex items-center justify-between">
        <div className="flex items-center gap-[7px]">
          <SolanaGlyph size={20} />
          <span className="text-[11px] font-semibold tracking-[0.04em] text-slate-300">
            {t("title")}
          </span>
        </div>
        <span className="flex items-center gap-[5px] text-[9px] font-semibold tracking-[0.18em] text-signal-good uppercase">
          <LiveDot />
          {t("live")}
        </span>
      </header>

      <dl className="mt-3 flex flex-col gap-[7px]">
        <StatRow label={t("slot")} value="—" />
        <StatRow label={t("latency")} value="—" />
      </dl>
    </div>
  );
}

function StatRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-baseline justify-between">
      <dt className="text-[10.5px] text-slate-500">{label}</dt>
      <dd className="font-mono text-[11px] text-slate-300">{value}</dd>
    </div>
  );
}

/** Pulsing status dot — a static dot under a ping-animated clone. */
function LiveDot() {
  return (
    <span className="relative h-1.5 w-1.5">
      <span className="absolute inset-0 animate-ping rounded-full bg-signal-good" />
      <span className="absolute inset-0 rounded-full bg-signal-good" />
    </span>
  );
}