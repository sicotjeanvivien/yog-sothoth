/** The `(developers)` layout: the dashboard's colours under a bar of its own, no menu. */

import { getTranslations } from "next-intl/server";

import { Link } from "@/i18n/navigation";
import { LogoMark } from "@/components/shared/logo-mark";

export default async function DevelopersLayout({ children }: { children: React.ReactNode }) {
  const t = await getTranslations("Developers.devbar");
  const tBrand = await getTranslations("Brand");

  return (
    <div className="dash-root min-h-screen bg-dash-bg font-dash-sans text-dash-ink [color-scheme:dark]">
      <header className="sticky top-0 z-30 flex min-h-14 items-center gap-3 border-b border-dash-rule bg-dash-bg px-4 rail:gap-4 rail:px-6">
        <Link href="/" aria-label={t("home")} className="flex items-center gap-3">
          <LogoMark width={30} priority />
          <span className="hidden font-display text-[16px] font-semibold tracking-[0.16em] whitespace-nowrap uppercase rail:inline">
            {tBrand("name")}
          </span>
        </Link>
        <span aria-hidden className="h-5 w-px bg-dash-rule" />
        <span className="font-dash-mono text-[12px] font-medium tracking-[0.12em] text-dash-ink-2 uppercase">
          {t("name")}
        </span>
      </header>
      <main>{children}</main>
    </div>
  );
}
