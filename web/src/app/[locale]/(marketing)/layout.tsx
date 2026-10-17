/**
 * The `(marketing)` layout: the top nav above the page, where the dashboard has a sidebar. Its
 * solid background covers the cosmic one of `body`.
 */

import { MarketingFooter } from "@/components/marketing/footer/marketing-footer";
import { MarketingNav } from "@/components/marketing/navbar/marketing-nav";

export default function MarketingLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <div className="site-root min-h-screen bg-dash-bg font-dash-sans text-dash-ink [color-scheme:dark]">
      <MarketingNav />
      {children}
      <MarketingFooter />
    </div>
  );
}