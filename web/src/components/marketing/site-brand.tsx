/** The site's logo and wordmark, back to the home page; the header's and the footer's. */

import { useTranslations } from "next-intl";

import { LogoMark } from "@/components/shared/logo-mark";
import { Link } from "@/i18n/navigation";

/** `compact`: a smaller logo up to 720 px, so the header bar holds on one line on a phone. */
export function SiteBrand({
  priority = false,
  compact = false,
}: {
  priority?: boolean;
  compact?: boolean;
}) {
  const t = useTranslations("Marketing");
  const tBrand = useTranslations("Brand");

  return (
    <Link
      href="/"
      aria-label={t("home")}
      className={`inline-flex min-h-11 items-center text-dash-ink ${compact ? "gap-2 rail:gap-3" : "gap-3"}`}
    >
      <LogoMark
        width={36}
        priority={priority}
        className={compact ? "h-auto w-8 rail:w-9" : ""}
      />
      <span className="font-display text-[16px] font-semibold tracking-[0.16em] uppercase">
        {tBrand("name")}
      </span>
    </Link>
  );
}
