"use client";

/**
 * The dashboard chrome: below `lg` the sidebar is a drawer behind a hamburger, above it a
 * fixed rail. The switch is CSS; React only tracks `isOpen`. No focus trap in the drawer.
 */

import { useCallback, useEffect, useState } from "react";
import { useTranslations } from "next-intl";

import { Sidebar } from "@/components/dashboard/sidebar/sidebar";
import {
  SIDEBAR_COLLAPSED_COOKIE,
  SIDEBAR_COOKIE_MAX_AGE_S,
} from "@/components/dashboard/sidebar/sidebar-state";
import { HamburgerIcon } from "@/components/shared/icon";

// ⚠️ Tailwind's `lg:`, repeated for the resize listener.
const LG_BREAKPOINT_PX = 1024;

export function DashboardShell({
  children,
  initialCollapsed = false,
  fontVariables,
}: {
  children: React.ReactNode;
  /** Read from the cookie by the server layout, so the first paint has the right width. */
  initialCollapsed?: boolean;
  /** next/font classes declaring `--font-geist` and `--font-geist-mono`. */
  fontVariables: string;
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

  // Reset to closed at `lg`, or the scroll-lock and overlay linger after a resize.
  useEffect(() => {
    const query = window.matchMedia(`(min-width: ${LG_BREAKPOINT_PX}px)`);
    function onChange(event: MediaQueryListEvent) {
      if (event.matches) close();
    }
    query.addEventListener("change", onChange);
    return () => query.removeEventListener("change", onChange);
  }, [close]);

  return (
    // Solid background over the cosmic one of `body` — behind the pages
    // that left the menu too.
    <div className={`${fontVariables} flex min-h-screen items-start bg-dash-bg`}>
      <MobileHeader onOpen={open} menuLabel={t("openMenu")} />

      {isOpen && <Overlay onClick={close} label={t("closeMenu")} />}

      <Sidebar
        isOpen={isOpen}
        onNavigate={close}
        collapsed={collapsed}
        onToggleCollapsed={toggleCollapsed}
      />

      {/* `pt-14` below `lg` makes room for the fixed mobile header. */}
      <main className="min-w-0 flex-1 pt-14 lg:pt-0">{children}</main>
    </div>
  );
}

function MobileHeader({
  onOpen,
  menuLabel,
}: {
  onOpen: () => void;
  menuLabel: string;
}) {
  return (
    <header className="fixed inset-x-0 top-0 z-30 flex h-14 items-center border-b border-dash-rule bg-dash-bg px-4 font-dash-sans lg:hidden">
      <button
        type="button"
        onClick={onOpen}
        aria-label={menuLabel}
        className="flex h-9 w-9 items-center justify-center text-dash-ink-2 transition-colors hover:bg-dash-surface hover:text-dash-ink"
      >
        <HamburgerIcon />
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
      className="fixed inset-0 z-30 bg-dash-bg/70 lg:hidden"
    />
  );
}