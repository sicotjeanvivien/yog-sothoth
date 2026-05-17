/**
 * Marketing footer.
 *
 * Shared marketing chrome — mounted by `(marketing)/layout.tsx`, so
 * it appears on every marketing page, the counterpart of
 * `MarketingNav`.
 *
 * # Layout
 *
 * Keeps the spirit of the mockup footer but sized to what the
 * product actually has (the mockup's four link columns assume pages
 * that don't exist yet). Three zones in the upper row:
 *
 *   1. brand   — logo, wordmark, one-line description, social icons;
 *   2. links   — a single short column (Overview / Features / Support);
 *   3. AWSD    — a "Built by AWSD" credit box, links to awsd.fr.
 *
 * A copyright bar sits below, separated by a hairline border.
 *
 * Static content, Server Component.
 *
 * # Assets
 *
 * Expects `web/public/logo.png` (Yog-Sothoth, already present) and
 * `web/public/awsd-logo.png` (AWSD — drop the file there; adjust the
 * src if your filename differs).
 */

import Image from "next/image";
import { useTranslations } from "next-intl";

import { Link } from "@/i18n/navigation";
import {
  FOOTER_AWSD,
  FOOTER_LINKS,
  FOOTER_SOCIALS,
} from "./marketing-footer-links";
import { GithubIcon, XIcon } from "@/components/shared/icon";

export function MarketingFooter() {
  const t = useTranslations("Marketing.footer");
  const year = new Date().getFullYear();

  return (
    <footer className="border-t border-sothoth-500/15 bg-cosmos-950">
      <div className="mx-auto max-w-[1800px] px-6 lg:px-12">
        {/* Upper row — brand / links / AWSD box */}
        <div className="grid grid-cols-1 gap-10 py-14 md:grid-cols-2 lg:grid-cols-[2fr_1fr_1.4fr]">
          <BrandBlock />
          <LinksColumn />
          <AwsdBox />
        </div>

        {/* Copyright bar */}
        <div className="border-t border-sothoth-500/10 py-6 text-center text-[12px] text-slate-500">
          {t("copyright", { year })}
        </div>
      </div>
    </footer>
  );
}

// ── Brand block ───────────────────────────────────────────────────────

/** Logo + wordmark + one-line description + social icons. */
function BrandBlock() {
  const t = useTranslations("Marketing.footer");
  const tBrand = useTranslations("Brand");

  return (
    <div>
      <Link href="/" className="flex items-center gap-[11px]">
        <div className="flex items-center text-letf">
          <Image
            src="/logo.png"
            alt={tBrand("name")}
            width={40}
            height={40}
            priority
            className="h-[40] w-[40] object-contain [filter:drop-shadow(0_0_14px_rgba(139,92,246,0.55))]"
          />
          <div className="flex flex-col ml-2">
            <p className="font-display text-[24px] font-semibold tracking-[0.22em] text-[#f1ecff] [text-indent:0.22em] [text-shadow:0_0_16px_rgba(139,92,246,0.75)]">
              {tBrand("name")}
            </p>
            <p className="text-[14px] font-semibold tracking-[0.34em] text-sothoth-500 uppercase [text-indent:0.34em]">
              {tBrand("tagline")}
            </p>
          </div>
        </div>
      </Link>

      <p className="mt-4 max-w-[280px] text-[17px] leading-[1.6] text-slate-400">
        {t("description")}
      </p>

      <div className="mt-5 flex items-center gap-3">
        {FOOTER_SOCIALS.map((social) => (
          <a
            key={social.key}
            href={social.href}
            target="_blank"
            rel="noopener noreferrer"
            aria-label={social.label}
            className="flex h-9 w-9 items-center justify-center rounded-[5px] border border-sothoth-500/15 text-slate-400 transition-colors hover:border-sothoth-500/35 hover:text-[#f1ecff]"
          >
            {social.icon === "x" ? <XIcon /> : <GithubIcon />}
          </a>
        ))}
      </div>
    </div>
  );
}

// ── Links column ──────────────────────────────────────────────────────

/**
 * The single column of footer links. Each entry is a route, an
 * in-page anchor, or an external URL — rendered accordingly.
 */
function LinksColumn() {
  const t = useTranslations("Marketing.footer");
  const tLinks = useTranslations("Marketing.footer.links");

  const itemClass =
    "text-[17px] font-medium text-slate-100 transition-colors hover:text-sothoth-400";

  return (
    <div>
      <h3 className="text-[14px] font-semibold tracking-[0.20em] text-slate-500 uppercase">
        {t("linksHeading")}
      </h3>

      <ul className="mt-4 flex flex-col gap-2.5">
        {FOOTER_LINKS.map((link) => {
          const label = tLinks(link.labelKey);

          // In-page fragment or external URL — plain anchor.
          if (link.kind === "anchor" || link.kind === "external") {
            const external = link.kind === "external";
            return (
              <li key={link.key}>
                <a
                  href={link.href}
                  className={itemClass}
                  {...(external
                    ? { target: "_blank", rel: "noopener noreferrer" }
                    : {})}
                >
                  {label}
                </a>
              </li>
            );
          }

          // Internal locale route.
          return (
            <li key={link.key}>
              <Link href={link.href} className={itemClass}>
                {label}
              </Link>
            </li>
          );
        })}
      </ul>
    </div>
  );
}

// ── AWSD credit box ───────────────────────────────────────────────────

/**
 * "Built by AWSD" box — the company credit. A single external link
 * to the AWSD site; the whole box is the click target.
 */
function AwsdBox() {
  const t = useTranslations("Marketing.footer.awsd");

  return (
    <a
      href={FOOTER_AWSD.href}
      target="_blank"
      rel="noopener noreferrer"
      className="group flex flex-col justify-center rounded-[8px] border border-sothoth-500/15 bg-cosmos-900/50 p-6 transition-colors hover:border-sothoth-500/35"
    >
      <div className="flex items-center gap-3">
        <Image
          src="/awsd-logo.png"
          alt="AWSD"
          width={40}
          height={40}
          className="h-[40] w-[40] object-contain"
        />
        <span className="font-display text-[20px] font-semibold tracking-[0.14em] text-[#f1ecff]">
          {t("title")}
        </span>
      </div>

      <p className="mt-3 text-[17px] leading-[1.6] text-slate-400">
        {t("tagline")}
      </p>
    </a>
  );
}
