/**
 * Shared CTA link.
 *
 * A single button-styled link used across marketing pages. Two
 * variants — `primary` (filled violet) and `secondary` (outlined
 * slate) — match the pair already in use on the homepage hero.
 *
 * Internal vs external is decided by the `external` flag:
 *
 *   - `external: true`  → plain `<a>` with `target="_blank"` and the
 *                         standard `rel` attributes. Used for GitHub,
 *                         awsd.fr, and any off-site target.
 *   - `external: false` (default) → next-intl's `Link`, so the
 *                         locale prefix is added automatically.
 *
 * An optional icon slot renders to the left of the label. Pass a
 * pre-sized icon component (e.g. `<GithubIcon className="h-[18px] w-[18px]" />`).
 *
 * Three sizes — `sm`, `md` (default), `lg` — adjust padding and font
 * size only; the visual identity is the same.
 */

import type { ReactNode } from "react";

import { Link } from "@/i18n/navigation";

// ── Variants & sizes ──────────────────────────────────────────────────

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

// ── Component ─────────────────────────────────────────────────────────

type CtaLinkProps = {
  href: string;
  label: string;
  /** Variant — defaults to `primary`. */
  variant?: Variant;
  /** Size — defaults to `md`. */
  size?: Size;
  /** Optional icon, rendered to the left of the label. */
  icon?: ReactNode;
  /**
   * Whether the target is off-site. Off-site → plain anchor with
   * `target="_blank"`; on-site → next-intl `Link`.
   */
  external?: boolean;
  /** Extra classes — appended last, so callers can override. */
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