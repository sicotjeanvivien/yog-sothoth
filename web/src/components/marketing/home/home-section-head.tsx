/** A home section's heading: eyebrow and title on the left, the intro on the right. */

import { SITE_LABEL_CLASS } from "@/components/marketing/site-label";

export const HOME_TITLE_CLASS =
  "text-[32px] leading-[1.05] font-semibold tracking-[-0.03em] rail:text-[44px]";

export function HomeSectionHead({
  id,
  eyebrow,
  title,
  intro,
}: {
  /** Of the title, for the section's `aria-labelledby`. */
  id: string;
  eyebrow: string;
  title: string;
  intro: string;
}) {
  return (
    <div className="mb-10 grid grid-cols-12 items-end gap-x-4 gap-y-4 rail:gap-x-8">
      <div className="col-span-12 flex flex-col gap-3.5 wide:col-span-7">
        <span className={SITE_LABEL_CLASS}>{eyebrow}</span>
        <h2 id={id} className={HOME_TITLE_CLASS}>
          {title}
        </h2>
      </div>
      <p className="col-span-12 text-[15px] text-dash-ink-2 wide:col-span-5">
        {intro}
      </p>
    </div>
  );
}
