/** The three free ways to help: star the repository, share the project, send feedback. */

import { useTranslations } from "next-intl";
import type { ReactNode } from "react";

import { PRIMARY_BUTTON_CLASS } from "@/components/shared/dashboard-button";
import { CONTACT_EMAIL_HREF } from "@/components/marketing/contact";
import { StarIcon } from "@/components/shared/icon";
import {
  SECONDARY_BUTTON_CLASS,
  SecondaryButton,
} from "@/components/shared/secondary-button";

const GITHUB_REPO_URL = "https://github.com/sicotjeanvivien/yog-sothoth";
const GITHUB_ISSUES_URL = `${GITHUB_REPO_URL}/issues`;
const SITE_URL = "https://yog-scope.xyz";

export function SupportUsWays() {
  const t = useTranslations("SupportUs.ways");

  const xShareUrl = `https://twitter.com/intent/tweet?text=${encodeURIComponent(
    t("shareText"),
  )}&url=${encodeURIComponent(SITE_URL)}`;
  const linkedinShareUrl = `https://www.linkedin.com/sharing/share-offsite/?url=${encodeURIComponent(
    SITE_URL,
  )}`;

  return (
    <section
      aria-label={t("label")}
      className="grid grid-cols-1 border-t border-l border-dash-rule wide:grid-cols-3"
    >
      <Way free={t("free")} title={t("star.title")} body={t("star.body")}>
        <a
          href={GITHUB_REPO_URL}
          target="_blank"
          rel="noopener noreferrer"
          className={PRIMARY_BUTTON_CLASS}
        >
          <StarIcon size={14} />
          {t("star.cta")}
        </a>
      </Way>
      <Way free={t("free")} title={t("share.title")} body={t("share.body")}>
        <SecondaryButton href={xShareUrl} label={t("share.ctaX")} external />
        <SecondaryButton href={linkedinShareUrl} label={t("share.ctaLinkedin")} external />
      </Way>
      <Way free={t("free")} title={t("feedback.title")} body={t("feedback.body")}>
        <SecondaryButton href={GITHUB_ISSUES_URL} label={t("feedback.ctaIssues")} external />
        <a href={CONTACT_EMAIL_HREF} className={SECONDARY_BUTTON_CLASS}>
          {t("feedback.ctaEmail")}
        </a>
      </Way>
    </section>
  );
}

function Way({
  free,
  title,
  body,
  children,
}: {
  free: string;
  title: string;
  body: string;
  children: ReactNode;
}) {
  return (
    <article className="flex min-w-0 flex-col gap-3.5 border-r border-b border-dash-rule bg-dash-surface px-6 py-7">
      <span className="self-start border border-dash-up px-[7px] py-0.5 font-dash-mono text-[10px] tracking-[0.1em] whitespace-nowrap text-dash-up uppercase">
        {free}
      </span>
      <h2 className="text-[22px] font-semibold tracking-[-0.015em]">{title}</h2>
      <p className="text-dash-ink-2">{body}</p>
      <div className="mt-auto flex flex-wrap gap-2.5 pt-2">{children}</div>
    </article>
  );
}
