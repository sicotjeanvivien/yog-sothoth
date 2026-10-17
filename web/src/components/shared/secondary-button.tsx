/** The site's outlined button, beside a `DashboardButton`; `href` is locale-free. */

import { Link } from "@/i18n/navigation";

export function SecondaryButton({
  href,
  label,
}: {
  href: string;
  label: string;
}) {
  return (
    <Link
      href={href}
      className="inline-flex min-h-11 items-center gap-2.5 border border-dash-rule px-[18px] text-[14px] font-medium whitespace-nowrap text-dash-ink transition-colors hover:border-dash-ink-3"
    >
      {label}
    </Link>
  );
}
