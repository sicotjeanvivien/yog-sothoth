"use client";

/**
 * Dashboard sidebar — persistent left rail.
 *
 * Autonomous Client Component:
 *   - reads the current route via next-intl's `usePathname` (returns
 *     the path *without* the locale segment, e.g. `/overview`);
 *   - resolves its own labels via `useTranslations`;
 *   - reads the nav structure from `sidebar-nav.ts`.
 *
 * The layout mounts it with no props: `<Sidebar />`.
 *
 * # Positioning
 *
 * The root `<aside>` is `sticky top-0 h-screen`. It stays in the flex
 * flow next to `<main>`, but sticks to the viewport top while the
 * page scrolls. For this to work the layout's flex container must not
 * trap the scroll in an `overflow` context — see `(dashboard)/layout`.
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

import type { ReactNode, FC } from "react";
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

/** A nav icon is any component accepting the shared `IconProps`. */
type IconComponent = (props: IconProps) => ReactNode;

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

export function Sidebar() {
  const pathname = usePathname();

  return (
    <aside className="sticky top-0 flex h-screen w-[248px] shrink-0 flex-col border-r border-sothoth-700/25 bg-cosmos-900 px-5 pt-8 pb-6">
      <BrandBlock />
      <Divider />
      <nav className="flex flex-1 flex-col gap-[3px]">
        <NavCaption />
        {SIDEBAR_NAV.map((item) => (
          <SidebarNavLink
            key={item.key}
            item={item}
            active={isItemActive(pathname, item.href)}
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
    <div className="flex flex-col items-center px-1 pt-1 pb-2 text-center">
      <Image
        src="/logo.png"
        alt={t("name")}
        width={168}
        height={168}
        priority
        className="object-contain [filter:drop-shadow(0_0_14px_rgba(139,92,246,0.55))]"
      />
      <p className="mt-3 font-display text-[17px] font-semibold tracking-[0.22em] text-[#f1ecff] [text-indent:0.22em] [text-shadow:0_0_16px_rgba(139,92,246,0.75)]">
        {t("name")}
      </p>
      <p className="mt-[7px] text-[9px] font-semibold tracking-[0.34em] text-sothoth-500 uppercase [text-indent:0.34em]">
        {t("tagline")}
      </p>
    </div>
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
 */
function SidebarNavLink({
  item,
  active,
}: {
  item: SidebarNavItem;
  active: boolean;
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