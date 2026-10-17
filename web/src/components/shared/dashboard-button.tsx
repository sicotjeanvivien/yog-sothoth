/** The call-to-action into the product: the accent fill, its text dark, never white. */

import { useTranslations } from "next-intl";

import { Link } from "@/i18n/navigation";
import { ArrowRightIcon } from "@/components/shared/icon";

export const PRIMARY_BUTTON_CLASS =
  "inline-flex min-h-11 items-center gap-2.5 border border-dash-accent bg-dash-accent px-[18px] text-[14px] font-semibold whitespace-nowrap text-dash-surface transition-colors hover:border-dash-accent-2 hover:bg-dash-accent-2";

export function DashboardButton({
  label,
  arrow = false,
}: {
  /** Overrides the default i18n label. */
  label?: string;
  arrow?: boolean;
}) {
  const t = useTranslations("Marketing");
  const text = label ?? t("dashboardButton");

  return (
    <Link
      href="/overview"
      className={PRIMARY_BUTTON_CLASS}
    >
      {text}
      {arrow && <ArrowRightIcon size={14} />}
    </Link>
  );
}
