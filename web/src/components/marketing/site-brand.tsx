/** The site's logo and wordmark, back to the home page; the header's and the footer's. */

import { useTranslations } from "next-intl";

import { LogoMark } from "@/components/shared/logo-mark";
import { Link } from "@/i18n/navigation";

export function SiteBrand({ priority = false }: { priority?: boolean }) {
  const t = useTranslations("Marketing");
  const tBrand = useTranslations("Brand");

  return (
    <Link
      href="/"
      aria-label={t("home")}
      className="inline-flex min-h-11 items-center gap-3 text-dash-ink"
    >
      <LogoMark width={36} priority={priority} />
      <span className="font-display text-[16px] font-semibold tracking-[0.16em] uppercase">
        {tBrand("name")}
      </span>
    </Link>
  );
}
