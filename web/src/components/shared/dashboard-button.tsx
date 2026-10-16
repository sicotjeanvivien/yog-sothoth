/**
 * The one call-to-action into the product, used by every "enter the app" link. ⚠️ The only
 * button with this gradient: a secondary action keeps the plain style and must not use it.
 */

import { useTranslations } from "next-intl";

import { Link } from "@/i18n/navigation";
import { ArrowRightIcon } from "./icon";

type DashboardButtonProps = {
  /** Overrides the default i18n label. */
  label?: string;
  /** `lg` for the hero and the closing CTA. */
  size?: "md" | "lg";
  className?: string;
};

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
      className={`inline-flex items-center gap-2 rounded-lg font-semibold text-[#f5f2ff] ${STYLE_CLASS} ${SIZE_CLASS[size]} ${className}`}
    >
      {text}
      <ArrowRightIcon />
    </Link>
  );
}
