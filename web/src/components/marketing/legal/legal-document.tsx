/** The one template of the three legal pages: tabs between them, a table of contents, numbered sections. */

import { useTranslations } from "next-intl";
import type { ReactNode } from "react";

import { Link } from "@/i18n/navigation";
import { PRIMARY_BUTTON_CLASS } from "@/components/shared/dashboard-button";
import { CONTACT_EMAIL_HREF } from "@/components/marketing/contact";
import { SITE_LABEL_CLASS } from "@/components/marketing/site-label";

export const LEGAL_LINK_CLASS =
  "text-dash-accent-2 underline decoration-dash-accent-2/40 underline-offset-4 hover:decoration-dash-accent-2";

/** `key` labels the tab under `Marketing.footer.company`; `namespace` holds the page's header. */
const DOCUMENTS = [
  { key: "privacy", href: "/privacy", namespace: "Privacy" },
  { key: "legal-notice", href: "/legal-notice", namespace: "LegalNotice" },
  { key: "terms", href: "/terms", namespace: "Terms" },
] as const;

export type LegalDocumentKey = (typeof DOCUMENTS)[number]["key"];

/** `id` is the section's anchor; `brief` sets it apart as the document's summary. */
export type LegalSection = {
  id: string;
  title: string;
  body: ReactNode;
  brief?: boolean;
};

const TOC_ID = "legal-toc";

export function LegalDocument({
  current,
  sections,
}: {
  current: LegalDocumentKey;
  sections: readonly LegalSection[];
}) {
  const t = useTranslations("Legal");
  const tTab = useTranslations("Marketing.footer.company");
  const { namespace } = DOCUMENTS.find(({ key }) => key === current)!;
  const tHeader = useTranslations(`${namespace}.header`);

  return (
    <>
      <nav
        aria-label={t("tabs")}
        className="flex flex-wrap gap-x-5 border-b border-dash-rule rail:gap-x-8"
      >
        {DOCUMENTS.map(({ key, href }) => (
          <Link
            key={key}
            href={href}
            aria-current={key === current ? "page" : undefined}
            className={`pt-[18px] pb-3.5 text-[14px] font-medium whitespace-nowrap rail:text-[15px] ${
              key === current
                ? "text-dash-ink shadow-[inset_0_-2px_0_var(--color-dash-accent)]"
                : "text-dash-ink-2 hover:text-dash-ink"
            }`}
          >
            {tTab(key)}
          </Link>
        ))}
      </nav>

      <header className="flex flex-col gap-4 border-b border-dash-rule pt-12 pb-8 rail:pt-[72px] rail:pb-10">
        <span className={SITE_LABEL_CLASS}>{tHeader("eyebrow")}</span>
        <h1 className="text-[38px] leading-[1.02] font-semibold tracking-[-0.03em] rail:text-[56px]">
          {tHeader("title")}
        </h1>
        <p className="flex flex-wrap gap-x-5 gap-y-2 text-[14px] text-dash-ink-2">
          <span>{tHeader("lastUpdated")}</span>
          <span>{t("count", { count: sections.length })}</span>
        </p>
      </header>

      <div className="grid grid-cols-12 gap-8 pt-12">
        <nav
          aria-labelledby={TOC_ID}
          className="col-span-12 flex flex-col gap-2.5 self-start wide:col-span-3"
        >
          <span id={TOC_ID} className={SITE_LABEL_CLASS}>
            {t("toc")}
          </span>
          <ol className="border-t border-dash-rule">
            {sections.map((section, i) => (
              <li key={section.id}>
                <a
                  href={`#${section.id}`}
                  className="grid min-h-11 grid-cols-[28px_minmax(0,1fr)] items-center gap-1.5 border-b border-dash-rule text-[14px] text-dash-ink-2 hover:bg-dash-surface hover:text-dash-ink"
                >
                  <span className="pl-1.5 font-dash-mono text-[11px] text-dash-ink-3">
                    {number(i)}
                  </span>
                  <span>{section.title}</span>
                </a>
              </li>
            ))}
          </ol>
        </nav>

        <article className="col-span-12 flex min-w-0 flex-col wide:col-span-7 wide:col-start-4">
          <p className="mb-8 max-w-[60ch] text-[17px] leading-[1.55] tracking-[-0.005em] rail:text-[19px]">
            {tHeader("lead")}
          </p>
          {sections.map((section, i) => (
            <section
              key={section.id}
              id={section.id}
              aria-labelledby={`${section.id}-title`}
              className="flex flex-col gap-3 border-t border-dash-rule py-7"
            >
              <div className="flex items-baseline gap-3.5">
                <span className="font-dash-mono text-[12px] text-dash-accent">{number(i)}</span>
                <h2
                  id={`${section.id}-title`}
                  className="text-[22px] font-semibold tracking-[-0.015em]"
                >
                  {section.title}
                </h2>
              </div>
              <div
                className={`text-[16px] leading-[1.65] ${
                  section.brief
                    ? "border border-dash-rule bg-dash-surface px-[22px] py-5 text-dash-ink"
                    : "max-w-[66ch] text-dash-ink-2"
                }`}
              >
                {section.body}
              </div>
            </section>
          ))}
          <div className="mt-6 flex flex-wrap items-center justify-between gap-x-6 gap-y-3 border border-dash-rule bg-dash-surface px-[22px] py-5">
            <p className="text-dash-ink-2">{t("question")}</p>
            <a href={CONTACT_EMAIL_HREF} className={PRIMARY_BUTTON_CLASS}>
              {t("write")}
            </a>
          </div>
        </article>
      </div>
    </>
  );
}

function number(index: number): string {
  return String(index + 1).padStart(2, "0");
}
