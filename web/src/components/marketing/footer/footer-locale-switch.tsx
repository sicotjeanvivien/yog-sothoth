"use client";

/** The footer's FR | EN: the same page in the other language. */

import { LocaleSwitch } from "@/components/shared/locale-switch";
import { usePathname } from "@/i18n/navigation";

export function FooterLocaleSwitch({ label }: { label: string }) {
  // ⚠️ The path alone: `useSearchParams` would need a Suspense on these static pages.
  const pathname = usePathname();

  return (
    <LocaleSwitch
      href={pathname}
      label={label}
      sizeClassName="min-h-11 min-w-12 rail:min-h-8 rail:min-w-10"
    />
  );
}
