/**
 * The one call-to-action into the product, used by every "enter the app" link: the accent
 * fill, its text dark, never white.
 */

import { useTranslations } from "next-intl";

import { Link } from "@/i18n/navigation";

export function DashboardButton({
  label,
}: {
  /** Overrides the default i18n label. */
  label?: string;
}) {
  const t = useTranslations("Marketing");
  const text = label ?? t("dashboardButton");

  return (
    <Link
      href="/overview"
      className="inline-flex min-h-11 items-center gap-2.5 border border-dash-accent bg-dash-accent px-[18px] text-[14px] font-semibold whitespace-nowrap text-dash-surface transition-colors hover:border-dash-accent-2 hover:bg-dash-accent-2"
    >
      {text}
    </Link>
  );
}
