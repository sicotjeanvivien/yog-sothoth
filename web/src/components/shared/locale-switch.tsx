/** FR | EN: each language a link to `href` in that language, the active one filled. */

import { useLocale } from "next-intl";

import { Link } from "@/i18n/navigation";
import { routing, type Locale } from "@/i18n/routing";

// Each language is named in itself, whatever the page's language.
const LANGUAGE_NAMES: Record<Locale, string> = {
  en: "English",
  fr: "Français",
};

export function LocaleSwitch({
  href,
  labelledBy,
  label,
  sizeClassName,
}: {
  /** The locale-free path, with its query if the caller keeps one. */
  href: string;
  labelledBy?: string;
  label?: string;
  sizeClassName: string;
}) {
  const locale = useLocale();

  return (
    <ul
      aria-labelledby={labelledBy}
      aria-label={label}
      className="inline-flex border border-dash-rule"
    >
      {routing.locales.map((target) => {
        const active = target === locale;
        return (
          <li key={target} className="border-r border-dash-rule last:border-r-0">
            <Link
              href={href}
              locale={target}
              lang={target}
              aria-label={LANGUAGE_NAMES[target]}
              aria-current={active ? "true" : undefined}
              className={`flex items-center justify-center px-2 font-dash-mono text-[11px] tracking-[0.06em] uppercase ${sizeClassName} ${active ? "bg-dash-accent font-semibold text-dash-surface" : "text-dash-ink-2 hover:text-dash-ink"}`}
            >
              {target}
            </Link>
          </li>
        );
      })}
    </ul>
  );
}
