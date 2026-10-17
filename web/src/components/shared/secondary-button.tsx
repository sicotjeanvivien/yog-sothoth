/**
 * The site's outlined button, beside a `DashboardButton`. `href` is locale-free, or a full URL
 * when `external` (a plain anchor in a new tab).
 */

import { Link } from "@/i18n/navigation";

const CLASS =
  "inline-flex min-h-11 items-center gap-2.5 border border-dash-rule px-[18px] text-[14px] font-medium whitespace-nowrap text-dash-ink transition-colors hover:border-dash-ink-3";

export function SecondaryButton({
  href,
  label,
  external = false,
}: {
  href: string;
  label: string;
  external?: boolean;
}) {
  if (external) {
    return (
      <a href={href} target="_blank" rel="noopener noreferrer" className={CLASS}>
        {label}
      </a>
    );
  }

  return (
    <Link href={href} className={CLASS}>
      {label}
    </Link>
  );
}
