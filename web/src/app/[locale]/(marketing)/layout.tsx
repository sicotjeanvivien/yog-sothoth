/** The `(marketing)` layout: the top nav above the page, where the dashboard has a sidebar. */

import { MarketingFooter } from "@/components/marketing/footer/marketing-footer";
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
      <MarketingFooter />
    </>
  );
}