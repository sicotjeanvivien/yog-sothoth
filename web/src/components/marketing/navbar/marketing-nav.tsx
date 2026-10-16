"use client";

/**
 * The marketing nav, above the hero rather than over it. Below `lg` the links and CTA
 * collapse behind a hamburger: the switch is CSS, React only tracks `isOpen`.
 */

import { useCallback, useEffect, useState } from "react";
import Image from "next/image";
import { useTranslations } from "next-intl";

import { Link } from "@/i18n/navigation";
import { MARKETING_NAV_LINKS } from "./marketing-nav-links";
import { CloseIcon, HamburgerIcon } from "@/components/shared/icon";
import { DashboardButton } from "@/components/shared/dashboard-button";

// ⚠️ Tailwind's `lg:`, repeated for the resize listener.
const LG_BREAKPOINT_PX = 1024;

export function MarketingNav() {
  const [isOpen, setIsOpen] = useState(false);

  const close = useCallback(() => setIsOpen(false), []);
  const toggle = useCallback(() => setIsOpen((v) => !v), []);

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

  // Reset to closed at `lg`, or the state sticks after a resize.
  useEffect(() => {
    const query = window.matchMedia(`(min-width: ${LG_BREAKPOINT_PX}px)`);
    function onChange(event: MediaQueryListEvent) {
      if (event.matches) close();
    }
    query.addEventListener("change", onChange);
    return () => query.removeEventListener("change", onChange);
  }, [close]);

  return (
    <nav className="relative z-40 border-b border-sothoth-500/15 bg-cosmos-950">
      <div className="relative z-40 mx-auto flex max-w-[1800px] items-center justify-between px-6 py-4.5 lg:px-12">
        <BrandMark />

        <div className="hidden items-center gap-9 lg:flex">
          <MarketingNavLinks orientation="row" />
        </div>

        <div className="hidden lg:block">
          <DashboardButton />
        </div>

        <button
          type="button"
          onClick={toggle}
          aria-expanded={isOpen}
          aria-label="Toggle navigation menu"
          className="flex h-9 w-9 items-center justify-center rounded-[3px] text-slate-300 transition-colors hover:bg-sothoth-500/10 hover:text-slate-100 lg:hidden"
        >
          {isOpen ? <CloseIcon /> : <HamburgerIcon />}
        </button>
      </div>

      <MobileMenu isOpen={isOpen} onClose={close} />
    </nav>
  );
}

/** The dropdown panel below `lg`, removed from the DOM when closed. */
function MobileMenu({
  isOpen,
  onClose,
}: {
  isOpen: boolean;
  onClose: () => void;
}) {
  if (!isOpen) return null;

  return (
    <div className="lg:hidden">
      {/* ⚠️ z-30, under the z-40 <nav>: the bar stays usable above the overlay. */}
      <button
        type="button"
        onClick={onClose}
        aria-label="Close navigation menu"
        className="fixed inset-0 z-30 bg-cosmos-950/70 backdrop-blur-[2px]"
      />

      <div className="relative z-40 border-b border-sothoth-500/15 bg-cosmos-950">
        <div className="flex flex-col gap-1 px-6 py-4">
          <MarketingNavLinks orientation="column" onNavigate={onClose} />
          <div className="mt-3">
            <DashboardButton className="w-full justify-center" />
          </div>
        </div>
      </div>
    </div>
  );
}

/** Logo + wordmark, links to the marketing home. */
function BrandMark() {
  const t = useTranslations("Brand");
  return (
    <Link href="/" className="flex items-center gap-2.75">
      <div className="flex items-center text-letf">
        <Image
          src="/logo_ys.png"
          alt={t("name")}
          width={48}
          height={54}
          priority
          className="h-13.5 w-12 object-contain filter-[drop-shadow(0_0_14px_rgba(139,92,246,0.55))]"
        />
        <div className="flex flex-col ml-2">
          <p className="font-display text-[24px] font-semibold tracking-[0.22em] text-[#f1ecff] indent-[0.22em] [text-shadow:0_0_16px_rgba(139,92,246,0.75)]">
            {t("name")}
          </p>
          <p className="text-[14px] font-semibold tracking-[0.34em] text-sothoth-500 uppercase indent-[0.34em]">
            {t("tagline")}
          </p>
        </div>
      </div>
    </Link>
  );
}

/** The nav links of both the desktop row and the mobile panel; `onNavigate` closes the panel. */
export function MarketingNavLinks({
  orientation,
  onNavigate,
}: {
  orientation: "row" | "column";
  onNavigate?: () => void;
}) {
  const t = useTranslations("Marketing.nav");

  const itemClass =
    orientation === "row"
      ? "text-[17px] font-medium text-slate-100 transition-colors hover:text-sothoth-400"
      : "rounded-[3px] px-3 py-[11px] text-[17px] font-medium text-slate-300 transition-colors hover:bg-sothoth-500/10 hover:text-[#f1ecff]";

  return (
    <>
      {MARKETING_NAV_LINKS.map((link) => {
        const label = t(link.labelKey);

        if (link.external) {
          return (
            <a
              key={link.key}
              href={link.href}
              target="_blank"
              rel="noopener noreferrer"
              onClick={onNavigate}
              className={itemClass}
            >
              {label}
            </a>
          );
        }

        return (
          <Link
            key={link.key}
            href={link.href}
            onClick={onNavigate}
            className={itemClass}
          >
            {label}
          </Link>
        );
      })}
    </>
  );
}
