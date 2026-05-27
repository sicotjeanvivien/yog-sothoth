/**
 * Support us — Feedback / Sponsor grid.
 *
 * Two side-by-side cards forming the second tier of the support
 * page:
 *
 *   - Feedback (left)  → email + GitHub issues. Costs the visitor
 *                        a few minutes. At v0.1 this is the most
 *                        useful kind of contribution after
 *                        visibility.
 *   - Sponsor  (right) → GitHub Sponsors + Solana wallet. Costs
 *                        the visitor money. A short note states
 *                        what the funds are used for, in line with
 *                        the project's transparency stance.
 *
 * On mobile the cards stack vertically.
 */

import { getTranslations } from "next-intl/server";
import type { FC, ReactNode } from "react";

import {
  GithubIcon,
  MailIcon,
  PulseIcon,
  ShieldIcon,
  type IconProps,
} from "@/components/shared/icon";

const GITHUB_ISSUES_URL = "https://github.com/sicotjeanvivien/yog-sothoth/issues";
const GITHUB_SPONSORS_URL = "https://github.com/sponsors/sicotjeanvivien";
const CONTACT_EMAIL_HREF = "mailto:[contact-email]";
const SOLANA_WALLET_ADDRESS = "[solana-wallet-address]";

const CARD_CLASS =
  "flex flex-col rounded-[6px] border border-sothoth-500/15 bg-cosmos-900/60 p-6 transition-colors hover:border-sothoth-500/35 lg:p-8";

const ICON_BADGE_CLASS =
  "inline-flex h-[40px] w-[40px] shrink-0 items-center justify-center rounded-[6px] border border-sothoth-500/25 bg-sothoth-600/10 text-sothoth-400";

const TITLE_CLASS =
  "font-display text-[18px] font-semibold tracking-[0.02em] text-[#f1ecff] lg:text-[20px]";

const BODY_CLASS = "mt-3 text-[15px] leading-[1.65] text-slate-300";

const CTA_PRIMARY_CLASS =
  "inline-flex items-center justify-center gap-2 rounded-[4px] border border-sothoth-500/45 bg-sothoth-600/15 px-5 py-[10px] text-[14px] font-semibold text-[#f1ecff] transition-colors hover:border-sothoth-500/70 hover:bg-sothoth-600/30";

const CTA_SECONDARY_CLASS =
  "inline-flex items-center justify-center gap-2 rounded-[4px] border border-slate-700 bg-transparent px-5 py-[10px] text-[14px] font-semibold text-slate-200 transition-colors hover:border-slate-500 hover:bg-slate-800/40";

const WALLET_CLASS =
  "mt-5 rounded-[4px] border border-sothoth-500/15 bg-cosmos-950/60 px-3 py-2 font-mono text-[12px] break-all text-slate-300";

// ── Component ─────────────────────────────────────────────────────────

export async function SupportUsActions() {

  return (
    <section className="mx-auto max-w-[1800px] px-6 pb-24 lg:px-12">
      <div className="mx-auto grid max-w-[64ch] grid-cols-1 gap-4 lg:max-w-none lg:grid-cols-2">
        <FeedbackCard />
        <SponsorCard />
      </div>
    </section>
  );
}

// ── Sub-components ────────────────────────────────────────────────────

async function FeedbackCard() {
  const t = await getTranslations("SupportUs.feedback");

  return (
    <CardShell Icon={PulseIcon} title={t("title")}>
      <p className={BODY_CLASS}>{t("body")}</p>
      <div className="mt-5 flex flex-wrap gap-3">
        <a href={CONTACT_EMAIL_HREF} className={CTA_PRIMARY_CLASS}>
          <MailIcon size={16} />
          {t("ctaEmail")}
        </a>
        <a
          href={GITHUB_ISSUES_URL}
          target="_blank"
          rel="noopener noreferrer"
          className={CTA_SECONDARY_CLASS}
        >
          <GithubIcon size={14} />
          {t("ctaIssues")}
        </a>
      </div>
    </CardShell>
  );
}

async function SponsorCard() {
  const t = await getTranslations("SupportUs.sponsor");

  return (
    <CardShell Icon={ShieldIcon} title={t("title")}>
      <p className={BODY_CLASS}>{t("body")}</p>
      <p className="mt-3 text-[14px] leading-[1.65] text-slate-400">
        {t("fundsNote")}
      </p>

      <div className="mt-5 flex flex-wrap gap-3">
        <a
          href={GITHUB_SPONSORS_URL}
          target="_blank"
          rel="noopener noreferrer"
          className={CTA_PRIMARY_CLASS}
        >
          <GithubIcon size={16} />
          {t("ctaGithub")}
        </a>
      </div>

      {/* Solana wallet — copy-friendly box. No copy-to-clipboard
          button on purpose; that would force the card to become a
          Client Component for a single nice-to-have. */}
      <div className="mt-6">
        <p className="text-[12px] font-semibold tracking-[0.2em] text-slate-400 uppercase">
          {t("solanaLabel")}
        </p>
        <div className={WALLET_CLASS}>{SOLANA_WALLET_ADDRESS}</div>
      </div>
    </CardShell>
  );
}

/**
 * Generic card shell shared by Feedback and Sponsor.
 */
function CardShell({
  Icon,
  title,
  children,
}: {
  Icon: FC<IconProps>;
  title: string;
  children: ReactNode;
}) {
  return (
    <article className={CARD_CLASS}>
      <div className="flex items-center gap-4">
        <div className={ICON_BADGE_CLASS}>
          <Icon size={20} />
        </div>
        <h2 className={TITLE_CLASS}>{title}</h2>
      </div>
      <div className="mt-2">{children}</div>
    </article>
  );
}