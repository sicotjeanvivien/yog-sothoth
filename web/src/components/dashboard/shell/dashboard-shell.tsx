"use client";

/**
 * The dashboard chrome: up to 720 px the sidebar is a drawer behind "Menu", from 721 px a
 * fixed rail. The switch is CSS; React only tracks `isOpen`. No focus trap in the drawer.
 */

import { useCallback, useEffect, useState } from "react";
import { useTranslations } from "next-intl";

import { Brand, Sidebar } from "@/components/dashboard/sidebar/sidebar";
import {
  SIDEBAR_COLLAPSED_COOKIE,
  SIDEBAR_COOKIE_MAX_AGE_S,
} from "@/components/dashboard/sidebar/sidebar-state";
import { RAIL_BREAKPOINT } from "@/components/shared/breakpoints";
import { HamburgerIcon } from "@/components/shared/icon";

export function DashboardShell({
  children,
  initialCollapsed = false,
  initialPreferencesOpen = false,
}: {
  children: React.ReactNode;
  /** Read from the cookie by the server layout, so the first paint has the right width. */
  initialCollapsed?: boolean;
  /** Read from its cookie by the server layout, like `initialCollapsed`. */
  initialPreferencesOpen?: boolean;
}) {
  const t = useTranslations("Dashboard.shell");
  const [isOpen, setIsOpen] = useState(false);
  const [collapsed, setCollapsed] = useState(initialCollapsed);

  const open = useCallback(() => setIsOpen(true), []);
  const close = useCallback(() => setIsOpen(false), []);

  // A cookie, not localStorage: the server layout renders the right width, with no flash.
  const toggleCollapsed = useCallback(() => {
    setCollapsed((value) => {
      const next = !value;
      document.cookie = `${SIDEBAR_COLLAPSED_COOKIE}=${next ? "1" : "0"}; path=/; max-age=${SIDEBAR_COOKIE_MAX_AGE_S}; samesite=lax`;
      return next;
    });
  }, []);

  useEffect(() => {
    if (!isOpen) return;
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") close();
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [isOpen, close]);

  useEffect(() => {
    if (!isOpen) return;
    const previous = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.body.style.overflow = previous;
    };
  }, [isOpen]);

  // Reset to closed on the rail, or the scroll-lock and overlay linger after a resize.
  useEffect(() => {
    const query = window.matchMedia(`(min-width: ${RAIL_BREAKPOINT})`);
    function onChange(event: MediaQueryListEvent) {
      if (event.matches) close();
    }
    query.addEventListener("change", onChange);
    return () => query.removeEventListener("change", onChange);
  }, [close]);

  return (
    // Solid background over the cosmic one of `body` — behind the pages
    // that left the menu too.
    <div
      className="dash-root flex min-h-screen items-start bg-dash-bg [color-scheme:dark]"
    >
      <MobileHeader onOpen={open} isOpen={isOpen} menu={t("menu")} />

      {isOpen && <Overlay onClick={close} label={t("closeMenu")} />}

      <Sidebar
        isOpen={isOpen}
        onNavigate={close}
        collapsed={collapsed}
        onToggleCollapsed={toggleCollapsed}
        initialPreferencesOpen={initialPreferencesOpen}
      />

      {/* `pt-14` up to 720 px makes room for the fixed mobile header. */}
      <main className="min-w-0 flex-1 pt-14 rail:pt-0">{children}</main>
    </div>
  );
}

function MobileHeader({
  onOpen,
  isOpen,
  menu,
}: {
  onOpen: () => void;
  isOpen: boolean;
  menu: string;
}) {
  return (
    <header className="fixed inset-x-0 top-0 z-30 flex h-14 items-center justify-between gap-3 border-b border-dash-rule bg-dash-bg px-4 font-dash-sans text-dash-ink rail:hidden">
      <Brand small />
      <button
        type="button"
        onClick={onOpen}
        aria-expanded={isOpen}
        aria-controls="dashboard-menu"
        className="inline-flex min-h-11 min-w-11 items-center justify-center gap-2 border border-dash-rule px-3 text-[14px] text-dash-ink transition-colors hover:bg-dash-surface"
      >
        <HamburgerIcon size={16} />
        {menu}
      </button>
    </header>
  );
}

function Overlay({ onClick, label }: { onClick: () => void; label: string }) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-label={label}
      className="fixed inset-0 z-30 bg-dash-bg/70 rail:hidden"
    />
  );
}
