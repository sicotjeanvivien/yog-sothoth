"use client";

/**
 * The site header. Up to 720 px the links fold behind "Menu", in the flow under the bar, and
 * the short "Dashboard" button stays out. The switch is CSS; React only tracks `isOpen`.
 */

import { useCallback, useEffect, useId, useRef, useState } from "react";
import { useTranslations } from "next-intl";

import { Link, usePathname } from "@/i18n/navigation";
import { RAIL_BREAKPOINT } from "@/components/shared/breakpoints";
import { CloseIcon, HamburgerIcon } from "@/components/shared/icon";
import { DashboardButton } from "@/components/shared/dashboard-button";
import { SiteBrand } from "@/components/marketing/site-brand";
import { MARKETING_NAV_LINKS } from "./marketing-nav-links";

export function MarketingNav() {
  const t = useTranslations("Marketing");
  const pathname = usePathname();
  // The page it was opened on: the layout outlives a navigation, any link closes it.
  const [openOn, setOpenOn] = useState<string | null>(null);
  const isOpen = openOn === pathname;
  const navId = useId();
  const toggleRef = useRef<HTMLButtonElement>(null);

  const close = useCallback(() => setOpenOn(null), []);
  const toggle = () => setOpenOn(isOpen ? null : pathname);

  useEffect(() => {
    if (!isOpen) return;
    function onKeyDown(event: KeyboardEvent) {
      if (event.key !== "Escape") return;
      close();
      // ⚠️ Focus may sit on a link of the panel being hidden.
      toggleRef.current?.focus();
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [isOpen, close]);

  // Reset to closed from the rail width, or the state sticks after a resize.
  useEffect(() => {
    const query = window.matchMedia(`(min-width: ${RAIL_BREAKPOINT})`);
    function onChange(event: MediaQueryListEvent) {
      if (event.matches) close();
    }
    query.addEventListener("change", onChange);
    return () => query.removeEventListener("change", onChange);
  }, [close]);

  return (
    <header className="border-b border-dash-rule">
      <div className="mx-auto flex min-h-16 max-w-[1280px] flex-wrap items-center justify-between gap-x-3 gap-y-2 rail:gap-x-8 px-4 rail:min-h-18 rail:px-8">
        <SiteBrand priority compact />

        <div className="flex items-center gap-2 rail:hidden">
          <DashboardButton label={t("dashboardShort")} />
          <button
            ref={toggleRef}
            type="button"
            onClick={toggle}
            aria-expanded={isOpen}
            aria-controls={navId}
            aria-label={t("nav.menu")}
            className="inline-flex min-h-11 min-w-11 items-center justify-center border border-dash-rule text-dash-ink"
          >
            {isOpen ? <CloseIcon size={16} /> : <HamburgerIcon size={16} />}
          </button>
        </div>

        <nav
          id={navId}
          aria-label={t("nav.label")}
          className={`${isOpen ? "flex" : "hidden"} basis-full flex-col border-t border-dash-rule pb-3 rail:flex rail:basis-auto rail:flex-row rail:flex-wrap rail:items-center rail:gap-x-7 rail:border-t-0 rail:pb-0`}
        >
          <MarketingNavLinks onNavigate={close} />
          {/* ⚠️ A wrapper: the button's own `inline-flex` would beat a `hidden` passed to it. */}
          <div className="hidden rail:block">
            <DashboardButton />
          </div>
        </nav>
      </div>
    </header>
  );
}

/** One row per link up to 720 px, inline from there; the current page is underlined. */
function MarketingNavLinks({ onNavigate }: { onNavigate: () => void }) {
  const t = useTranslations("Marketing.nav");
  const pathname = usePathname();

  const itemClass =
    "inline-flex min-h-12 items-center border-b border-dash-rule text-[16px] transition-colors hover:text-dash-ink rail:min-h-11 rail:border-b-0 rail:text-[14px]";

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
              className={`${itemClass} text-dash-ink-2`}
            >
              {label}
            </a>
          );
        }

        const current = pathname === link.href;
        return (
          <Link
            key={link.key}
            href={link.href}
            onClick={onNavigate}
            aria-current={current ? "page" : undefined}
            className={`${itemClass} ${current ? "text-dash-ink shadow-[inset_0_-2px_0_var(--color-dash-accent)]" : "text-dash-ink-2"}`}
          >
            {label}
          </Link>
        );
      })}
    </>
  );
}
