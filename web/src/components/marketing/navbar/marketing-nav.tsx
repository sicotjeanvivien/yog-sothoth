/**
 * Marketing navigation — top banner.
 *
 * Its own block: solid cosmos background, hairline bottom border.
 * NOT overlaid on the hero image — it sits above it in the flow.
 *
 * Mounted by `(marketing)/layout.tsx`, so it appears on every
 * marketing page (homepage, support, …).
 *
 * # Scope of this commit
 *
 * Desktop layout only. The links row is hidden below lg — the mobile
 * hamburger menu is a separate, follow-up commit. The structure here
 * already isolates `MarketingNavLinks` as its own sub-component so
 * the mobile menu can reuse it without reshaping this file.
 *
 * # Internal vs external links
 *
 * Links come from `MARKETING_NAV_LINKS`. Each carries an `external`
 * flag: internal entries render through next-intl's `Link` (locale
 * prefix handled automatically), external entries render as a plain
 * anchor opened in a new tab with `rel="noopener noreferrer"`.
 */

import Image from "next/image";
import { useTranslations } from "next-intl";

import { Link } from "@/i18n/navigation";
import {
  MARKETING_NAV_CTA,
  MARKETING_NAV_LINKS,
} from "./marketing-nav-links";

export function MarketingNav() {
  return (
    <nav className="border-b border-sothoth-500/15 bg-cosmos-950">
      <div className="mx-auto flex max-w-[1800px] items-center justify-between px-6 py-[18px] lg:px-12">
        <BrandMark />

        {/* Desktop links — hidden below lg (mobile menu is commit 2). */}
        <div className="hidden items-center gap-9 lg:flex">
          <MarketingNavLinks />
        </div>

        {/* CTA — always visible. */}
        <div className="hidden lg:block">
          <NavCta />
        </div>
      </div>
    </nav>
  );
}

// ── Sub-components ────────────────────────────────────────────────────

/** Logo + wordmark, links to the marketing home. */
function BrandMark() {
  const t = useTranslations("Brand");
  return (
    <Link href="/" className="flex items-center gap-[11px]">
      <Image
        src="/logo.png"
        alt={t("name")}
        width={34}
        height={34}
        priority
        className="h-[34px] w-[34px] object-contain [filter:drop-shadow(0_0_8px_rgba(139,92,246,0.55))]"
      />
      <span className="font-display text-[14.5px] font-semibold tracking-[0.16em] text-[#f1ecff] [text-indent:0.16em]">
        {t("name")}
      </span>
    </Link>
  );
}

/**
 * The row of nav links. Isolated as its own component so the
 * follow-up mobile-menu commit can render the same list inside the
 * dropdown panel without duplicating the internal/external logic.
 */
export function MarketingNavLinks() {
  const t = useTranslations("Marketing.nav");

  return (
    <>
      {MARKETING_NAV_LINKS.map((link) => {
        const label = t(link.labelKey);
        const className =
          "text-[13.5px] font-medium text-slate-400 transition-colors hover:text-[#f1ecff]";

        // External links bypass the locale router — plain anchor,
        // new tab, with the security rel attributes.
        if (link.external) {
          return (
            <a
              key={link.key}
              href={link.href}
              target="_blank"
              rel="noopener noreferrer"
              className={className}
            >
              {label}
            </a>
          );
        }

        return (
          <Link key={link.key} href={link.href} className={className}>
            {label}
          </Link>
        );
      })}
    </>
  );
}

/** Primary call-to-action button. Internal, locale-aware. */
function NavCta() {
  const t = useTranslations("Marketing.nav");
  return (
    <Link
      href={MARKETING_NAV_CTA.href}
      className="inline-flex items-center gap-2 rounded-[4px] border border-sothoth-500/45 bg-sothoth-600/15 px-5 py-[9px] text-[13px] font-semibold text-[#f1ecff] transition-colors hover:border-sothoth-500/70 hover:bg-sothoth-600/30"
    >
      {t(MARKETING_NAV_CTA.labelKey)}
      <ArrowRightIcon />
    </Link>
  );
}

/** Small right arrow used on CTAs. */
function ArrowRightIcon() {
  return (
    <svg
      width={14}
      height={14}
      viewBox="0 0 20 20"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.8}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d="M4 10h12M11 5l5 5-5 5" />
    </svg>
  );
}