/**
 * Layout for the marketing route group `(marketing)`.
 *
 * Mounts the marketing nav above the page content. The route group
 * does not appear in the URL, so pages keep their natural paths
 * (`/[locale]`, `/[locale]/support`, …) while sharing this chrome.
 *
 * Counterpart to `(dashboard)/layout.tsx` — the two route groups
 * have deliberately separate chrome: a sidebar for the dashboard,
 * a top nav for marketing.
 *
 * NOTE: merge this with your existing `(marketing)/layout.tsx` —
 * keep whatever locale/metadata wiring you already have, just add
 * the `<MarketingNav />` above `{children}`.
 */

import { MarketingNav } from "@/components/marketing/navbar/marketing-nav";

export default function MarketingLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <>
      <MarketingNav />
      {children}
    </>
  );
}