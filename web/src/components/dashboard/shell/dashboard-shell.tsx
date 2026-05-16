"use client";

/**
 * DashboardShell — responsive chrome orchestrator.
 *
 * Owns the one piece of state the sidebar layout needs: whether the
 * mobile drawer is open. Everything that depends on that state lives
 * here so the `Sidebar` component itself stays a near-pure rail.
 *
 * # Two modes, one breakpoint (lg = 1024px)
 *
 *   >= lg : the sidebar is a permanent fixed rail; no header, no
 *           overlay, the drawer state is irrelevant.
 *   <  lg : the sidebar becomes a drawer — off-canvas by default,
 *           slid in over an overlay when `isOpen`. A compact header
 *           with a hamburger button is shown at the top.
 *
 * The mode switch is purely CSS (Tailwind `lg:` variants). React only
 * tracks `isOpen`; it never measures the viewport to decide the mode.
 *
 * # Behaviours wired here
 *
 *   - open / close via the hamburger and the overlay;
 *   - close on navigation (passed to the sidebar as `onNavigate`) so
 *     tapping a link doesn't leave the drawer open on the new page;
 *   - close on Escape;
 *   - body scroll-lock while the drawer is open;
 *   - reset to closed when the viewport grows to >= lg, so the state
 *     can't get stuck inconsistent after a resize.
 *
 * Focus trapping inside the open drawer is intentionally NOT done in
 * this iteration — it is a separate accessibility pass.
 */

import { useCallback, useEffect, useState } from "react";
import { useTranslations } from "next-intl";

import { Sidebar } from "@/components/dashboard/sidebar/sidebar";
import { HamburgerIcon } from "@/components/shared/icon";

// The drawer/fixed switch happens at this width. Kept as a constant
// for the resize listener; the CSS side uses Tailwind's `lg:`.
const LG_BREAKPOINT_PX = 1024;

export function DashboardShell({ children }: { children: React.ReactNode }) {
  const t = useTranslations("Dashboard.shell");
  const [isOpen, setIsOpen] = useState(false);

  const open = useCallback(() => setIsOpen(true), []);
  const close = useCallback(() => setIsOpen(false), []);

  // ── Close on Escape ─────────────────────────────────────────────────
  useEffect(() => {
    if (!isOpen) return;
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") close();
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [isOpen, close]);

  // ── Body scroll-lock while the drawer is open ───────────────────────
  useEffect(() => {
    if (!isOpen) return;
    const previous = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.body.style.overflow = previous;
    };
  }, [isOpen]);

  // ── Reset to closed once the viewport reaches the fixed-rail mode ───
  // Without this, opening the drawer on mobile then widening the
  // window to >= lg would leave `isOpen` true — harmless visually
  // (the drawer CSS is overridden by `lg:`), but the scroll-lock and
  // overlay would linger. Clearing the state keeps things coherent.
  useEffect(() => {
    const query = window.matchMedia(`(min-width: ${LG_BREAKPOINT_PX}px)`);
    function onChange(event: MediaQueryListEvent) {
      if (event.matches) close();
    }
    query.addEventListener("change", onChange);
    return () => query.removeEventListener("change", onChange);
  }, [close]);

  return (
    <div className="flex min-h-screen items-start">
      {/*
       * Mobile header — only shown below lg. Carries the hamburger.
       * On lg+ it collapses to nothing (`lg:hidden`).
       */}
      <MobileHeader onOpen={open} menuLabel={t("openMenu")} />

      {/*
       * Overlay — the dark scrim behind the open drawer. Only
       * rendered when open, and only visible below lg.
       */}
      {isOpen && <Overlay onClick={close} label={t("closeMenu")} />}

      {/* The sidebar itself — fixed rail on lg+, drawer below. */}
      <Sidebar isOpen={isOpen} onNavigate={close} />

      {/*
       * Main content. `pt-14` below lg leaves room for the fixed
       * mobile header; on lg+ the header is gone so no padding.
       */}
      <main className="min-w-0 flex-1 pt-14 lg:pt-0">{children}</main>
    </div>
  );
}

// ── Sub-components (private) ──────────────────────────────────────────

/**
 * Compact top bar shown below lg. Fixed to the viewport top, holds
 * the hamburger button that opens the drawer. Hidden on lg+.
 */
function MobileHeader({
  onOpen,
  menuLabel,
}: {
  onOpen: () => void;
  menuLabel: string;
}) {
  return (
    <header className="fixed inset-x-0 top-0 z-30 flex h-14 items-center border-b border-sothoth-700/25 bg-cosmos-900/95 px-4 backdrop-blur-sm lg:hidden">
      <button
        type="button"
        onClick={onOpen}
        aria-label={menuLabel}
        className="flex h-9 w-9 items-center justify-center rounded-[3px] text-slate-300 transition-colors hover:bg-sothoth-500/10 hover:text-slate-100"
      >
        <HamburgerIcon />
      </button>
    </header>
  );
}

/**
 * Dark scrim behind the open drawer. Clicking it closes the drawer.
 * Visible only below lg — on lg+ the drawer never opens, but the
 * `lg:hidden` guard keeps it out of the way regardless.
 */
function Overlay({ onClick, label }: { onClick: () => void; label: string }) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-label={label}
      className="fixed inset-0 z-30 bg-cosmos-950/70 backdrop-blur-[2px] lg:hidden"
    />
  );
}