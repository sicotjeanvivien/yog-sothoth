/** A site page's header: eyebrow and title on the left, the lead on the right. */

import { SITE_GRID_CLASS } from "@/components/marketing/site-grid";
import { SITE_LABEL_CLASS } from "@/components/marketing/site-label";

export function SitePageHeader({
  eyebrow,
  title,
  lead,
}: {
  eyebrow: string;
  title: string;
  lead: string;
}) {
  return (
    <header className={`${SITE_GRID_CLASS} items-end gap-y-5 pt-12 pb-8 rail:pt-[88px] rail:pb-10`}>
      <div className="col-span-12 flex flex-col gap-4.5 wide:col-span-7">
        <span className={SITE_LABEL_CLASS}>{eyebrow}</span>
        <h1 className="text-[42px] leading-none font-semibold tracking-[-0.035em] rail:text-[64px]">
          {title}
        </h1>
      </div>
      <p className="col-span-12 text-[17px] text-dash-ink-2 wide:col-span-5">{lead}</p>
    </header>
  );
}
