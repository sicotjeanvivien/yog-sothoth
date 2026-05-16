"use client";

/**
 * Marketing navigation — top banner, responsive.
 *
 * Its own block: solid cosmos background, hairline bottom border.
 * NOT overlaid on the hero — it sits above it in the flow. Mounted by
 * `(marketing)/layout.tsx`, so it appears on every marketing page.
 *
 * # Two modes at the lg breakpoint (1024px)
 *
 *   >= lg : the full row — links + CTA — is shown inline.
 *   <  lg : links + CTA collapse behind a hamburger button. Tapping
 *           it opens a dropdown panel under the bar, over a dark
 *           overlay. The panel is rendered only while open.
 *
 * The mode switch is pure CSS (`lg:` variants). React only tracks
 * `isOpen` for the mobile dropdown.
 *
 * # Behaviours wired here
 *
 *   - toggle via the hamburger;
 *   - close on link click (so navigating doesn't leave it open);
 *   - close on Escape;
 *   - body scroll-lock while open;
 *   - reset to closed when the viewport reaches lg, so the state
 *     can't get stuck after a resize.
 *
 * # Internal vs external links
 *
 * Links come from `MARKETING_NAV_LINKS`. Each carries an `external`
 * flag: internal entries render through next-intl's `Link` (locale
 * prefix handled automatically), external entries render as a plain
 * anchor opened in a new tab.
 *
 * `MarketingNavLinks` is shared between the desktop row and the
 * mobile panel — same links, different layout via the `orientation`
 * prop.
 */

import { useCallback, useEffect, useState } from "react";
import Image from "next/image";
import { useTranslations } from "next-intl";

import { Link } from "@/i18n/navigation";
import {
  MARKETING_NAV_CTA,
  MARKETING_NAV_LINKS,
} from "./marketing-nav-links";
import { ArrowRightIcon, CloseIcon, HamburgerIcon } from "@/components/shared/icon";

// The dropdown/inline switch happens at this width. Kept as a
// constant for the resize listener; the CSS side uses Tailwind `lg:`.
const LG_BREAKPOINT_PX = 1024;

export function MarketingNav() {
  const [isOpen, setIsOpen] = useState(false);

  const close = useCallback(() => setIsOpen(false), []);
  const toggle = useCallback(() => setIsOpen((v) => !v), []);

  // ── Close on Escape ─────────────────────────────────────────────────
  useEffect(() => {
    if (!isOpen) return;
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") close();
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [isOpen, close]);

  // ── Body scroll-lock while the panel is open ────────────────────────
  useEffect(() => {
    if (!isOpen) return;
    const previous = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.body.style.overflow = previous;
    };
  }, [isOpen]);

  // ── Reset to closed once the viewport reaches the inline mode ───────
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
      <div className="relative z-40 mx-auto flex max-w-[1800px] items-center justify-between px-6 py-[18px] lg:px-12">
        <BrandMark />

        {/* Desktop links — inline, lg+ only. */}
        <div className="hidden items-center gap-9 lg:flex">
          <MarketingNavLinks orientation="row" />
        </div>

        {/* Desktop CTA — lg+ only. */}
        <div className="hidden lg:block">
          <NavCta />
        </div>

        {/* Mobile hamburger — below lg only. */}
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

      {/* Mobile dropdown panel + overlay. */}
      <MobileMenu isOpen={isOpen} onClose={close} />
    </nav>
  );
}

// ── Mobile menu ───────────────────────────────────────────────────────

/**
 * The dropdown panel shown below lg. Rendered only while open —
 * removed from the DOM when closed, which makes the closed state
 * unambiguous (no reliance on max-height/opacity collapsing).
 *
 * No slide animation as a result: the panel appears directly. A
 * follow-up could reintroduce an animated reveal with a grid-rows
 * technique if desired.
 */
function MobileMenu({
  isOpen,
  onClose,
}: {
  isOpen: boolean;
  onClose: () => void;
}) {
  // Closed: render nothing. The hamburger in the bar is the only
  // affordance, and the bar itself stays untouched.
  if (!isOpen) return null;

  return (
    <div className="lg:hidden">
      {/*
       * Overlay — full-screen, but sits BELOW the nav bar in the
       * stacking order (the <nav> is z-40, this is z-30) so the bar
       * stays visible and interactive above it. Click closes.
       */}
      <button
        type="button"
        onClick={onClose}
        aria-label="Close navigation menu"
        className="fixed inset-0 z-30 bg-cosmos-950/70 backdrop-blur-[2px]"
      />

      {/* Panel — sits directly under the bar, above the overlay. */}
      <div className="relative z-40 border-b border-sothoth-500/15 bg-cosmos-950">
        <div className="flex flex-col gap-1 px-6 py-4">
          <MarketingNavLinks orientation="column" onNavigate={onClose} />
          <div className="mt-3">
            <NavCta onNavigate={onClose} />
          </div>
        </div>
      </div>
    </div>
  );
}

// ── Shared sub-components ─────────────────────────────────────────────

/** Logo + wordmark, links to the marketing home. */
function BrandMark() {
  const t = useTranslations("Brand");
  return (
    <Link href="/" className="flex items-center gap-[11px]">
      <div className="flex items-center text-letf">
        <Image
          src="/logo.png"
          alt={t("name")}
          width={84}
          height={84}
          priority
          className="h-[auto] w-[auto] object-contain [filter:drop-shadow(0_0_14px_rgba(139,92,246,0.55))]"
        />
        <div className="flex flex-col">
          <p className="font-display text-[17px] font-semibold tracking-[0.22em] text-[#f1ecff] [text-indent:0.22em] [text-shadow:0_0_16px_rgba(139,92,246,0.75)]">
            {t("name")}
          </p>
          <p className="text-[9px] font-semibold tracking-[0.34em] text-sothoth-500 uppercase [text-indent:0.34em]">
            {t("tagline")}
          </p>
        </div>
      </div>
    </Link>
  );
}

/**
 * The nav links. Shared between the desktop row and the mobile
 * dropdown — `orientation` only affects per-item layout; the
 * internal/external link logic is identical in both.
 *
 * `onNavigate` (used by the mobile panel) closes the menu when a
 * link is clicked.
 */
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
      ? "text-[14.5px] font-medium text-slate-400 transition-colors hover:text-[#f1ecff]"
      : "rounded-[3px] px-3 py-[11px] text-[14px] font-medium text-slate-300 transition-colors hover:bg-sothoth-500/10 hover:text-[#f1ecff]";

  return (
    <>
      {MARKETING_NAV_LINKS.map((link) => {
        const label = t(link.labelKey);

        // External links bypass the locale router — plain anchor,
        // new tab, with the security rel attributes.
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

/** Primary call-to-action button. Internal, locale-aware. */
function NavCta({ onNavigate }: { onNavigate?: () => void }) {
  const t = useTranslations("Marketing.nav");
  return (
    <Link
      href={MARKETING_NAV_CTA.href}
      onClick={onNavigate}
      className="inline-flex items-center gap-2 rounded-[4px] border border-sothoth-500/45 bg-sothoth-600/15 px-5 py-[9px] text-[13px] font-semibold text-[#f1ecff] transition-colors hover:border-sothoth-500/70 hover:bg-sothoth-600/30"
    >
      {t(MARKETING_NAV_CTA.labelKey)}
      <ArrowRightIcon />
    </Link>
  );
}
