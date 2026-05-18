/**
 * Dashboard button — the single, identifiable call-to-action into
 * the product.
 *
 * Every "enter the app" link across the site routes through this one
 * component: the marketing nav, the hero, the closing CTA, and any
 * future surface. It is deliberately the ONLY button with this
 * gradient treatment — that distinctiveness is the point. Secondary
 * actions (e.g. the hero's "See features") keep the plain
 * translucent style and must NOT use this component.
 *
 * Lives in `shared/` because it serves both the marketing pages and,
 * potentially, the dashboard surface.
 *
 * # Style
 *
 * Built to match the reference mockup, layered:
 *
 *   - fill    — a DIAGONAL gradient (135°): vivid violet at the
 *               top-left fading to a near-black violet at the
 *               bottom-right;
 *   - rim     — a 1px luminous lavender border, the "catches the
 *               light" edge;
 *   - top hl  — an inset highlight along the upper edge, so light
 *               reads as falling from above;
 *   - glow    — a soft external violet halo.
 *
 * On hover the fill lightens slightly and the glow intensifies.
 *
 * Every layer is defined here once — change the look in this single
 * place and every CTA on the site updates.
 *
 * # Label
 *
 * Defaults to a single i18n string (`Marketing.dashboardButton`), so
 * the button reads the same everywhere. The `label` prop can
 * override it if a specific surface ever needs different wording.
 *
 * # Sizes
 *
 * `md` is the default (nav, inline use); `lg` is for prominent
 * placements (hero, closing CTA).
 */
 
import { useTranslations } from "next-intl";
 
import { Link } from "@/i18n/navigation";
import { ArrowRightIcon } from "./icon";
 
type DashboardButtonProps = {
  /** Overrides the default i18n label. */
  label?: string;
  /** Visual size. Defaults to "md". */
  size?: "md" | "lg";
  /** Extra classes appended to the button (layout tweaks, etc.). */
  className?: string;
};
 
/*
 * The visual identity, as one class string.
 *
 * - `bg-[linear-gradient(135deg,…)]` — diagonal fill, vivid violet
 *   (#8b5cf6) at top-left through #5b21b6 to near-black violet
 *   (#1e1035) at bottom-right.
 * - `border` + `border-[…]` — the 1px luminous lavender rim.
 * - `shadow-[…]` — two shadows in one declaration: an *inset* light
 *   line along the top edge (the highlight) and an external violet
 *   *glow*.
 * - `hover:` — a lighter gradient and a stronger glow on hover.
 */
const STYLE_CLASS = [
  // base fill
  "bg-[linear-gradient(135deg,#8b5cf6_0%,#5b21b6_48%,#1e1035_100%)]",
  // luminous rim
  "border border-[rgba(196,181,253,0.45)]",
  // inset top highlight + external glow
  "shadow-[inset_0_1px_0_rgba(221,214,254,0.45),0_0_22px_rgba(124,58,237,0.45)]",
  // hover — lighter fill, brighter glow
  "hover:bg-[linear-gradient(135deg,#a78bfa_0%,#6d28d9_48%,#2a1745_100%)]",
  "hover:shadow-[inset_0_1px_0_rgba(221,214,254,0.6),0_0_32px_rgba(124,58,237,0.65)]",
  "transition-all duration-150",
].join(" ");
 
const SIZE_CLASS: Record<NonNullable<DashboardButtonProps["size"]>, string> = {
  md: "px-5 py-[10px] text-[14px]",
  lg: "px-6 py-3 text-[17px]",
};
 
export function DashboardButton({
  label,
  size = "md",
  className = "",
}: DashboardButtonProps) {
  const t = useTranslations("Marketing");
  const text = label ?? t("dashboardButton");
 
  return (
    <Link
      href="/overview"
      className={`inline-flex items-center gap-2 rounded-[8px] font-semibold text-[#f5f2ff] ${STYLE_CLASS} ${SIZE_CLASS[size]} ${className}`}
    >
      {text}
      <ArrowRightIcon />
    </Link>
  );
}