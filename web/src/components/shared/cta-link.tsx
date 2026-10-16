/**
 * The marketing CTA link, `primary` or `secondary`. `external` gives a plain `<a>` in a new
 * tab; otherwise next-intl's `Link` adds the locale.
 */

import type { ReactNode } from "react";

import { Link } from "@/i18n/navigation";

type Variant = "primary" | "secondary";
type Size = "sm" | "md" | "lg";

const BASE_CLASS =
  "inline-flex items-center gap-2 rounded-[4px] border font-semibold transition-colors";

const VARIANT_CLASS: Record<Variant, string> = {
  primary:
    "border-sothoth-500/45 bg-sothoth-600/15 text-[#f1ecff] hover:border-sothoth-500/70 hover:bg-sothoth-600/30",
  secondary:
    "border-slate-700 bg-transparent text-slate-200 hover:border-slate-500 hover:bg-slate-800/40",
};

const SIZE_CLASS: Record<Size, string> = {
  sm: "px-4 py-[8px] text-[14px]",
  md: "px-5 py-[11px] text-[15px]",
  lg: "px-5 py-[11px] text-[17px]",
};

type CtaLinkProps = {
  href: string;
  label: string;
  variant?: Variant;
  size?: Size;
  /** A pre-sized icon, left of the label. */
  icon?: ReactNode;
  external?: boolean;
  /** Appended last, so it overrides. */
  className?: string;
};

export function CtaLink({
  href,
  label,
  variant = "primary",
  size = "md",
  icon,
  external = false,
  className = "",
}: CtaLinkProps) {
  const classes = [
    BASE_CLASS,
    VARIANT_CLASS[variant],
    SIZE_CLASS[size],
    className,
  ]
    .filter(Boolean)
    .join(" ");

  const content = (
    <>
      {icon}
      {label}
    </>
  );

  if (external) {
    return (
      <a
        href={href}
        target="_blank"
        rel="noopener noreferrer"
        className={classes}
      >
        {content}
      </a>
    );
  }

  return (
    <Link href={href} className={classes}>
      {content}
    </Link>
  );
}