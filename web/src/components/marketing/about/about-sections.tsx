/** The About page's five numbered sections, copy under `About.sections` in `messages/{en,fr}/about.json`. */

import Image from "next/image";
import { useTranslations } from "next-intl";
import type { ReactNode } from "react";

import { SecondaryButton } from "@/components/shared/secondary-button";
import { SITE_LABEL_CLASS } from "@/components/marketing/site-label";
import { AboutScope } from "./about-scope";

export const GITHUB_REPO_URL = "https://github.com/sicotjeanvivien/yog-sothoth";
const AWSD_URL = "https://awsd.fr/";

/** ⚠️ The target of the hero's "Who is behind it" link. */
export const BEHIND_ID = "awsd";

// DAMM v2 instruction names, as the code spells them: not translated.
const EVENT_KINDS = [
  "swap",
  "add_liquidity",
  "remove_liquidity",
  "claim_position_fee",
  "claim_reward",
] as const;

const PIPE_KEYS = ["capture", "enrich", "store"] as const;
const AUDIENCE_KEYS = ["lp", "market", "api"] as const;
const TERM_KEYS = ["license", "code", "dashboard"] as const;

const BODY_CLASS = "max-w-[64ch] text-[16px] leading-[1.6] text-dash-ink-2";
/** A grid of bordered cells: the outer border comes from the cells' right and bottom edges. */
const CELLS_CLASS = "grid grid-cols-1 border-t border-l border-dash-rule";
const CELL_CLASS = "flex flex-col border-r border-b border-dash-rule bg-dash-surface";

export function AboutSections() {
  const t = useTranslations("About.sections");

  return (
    <>
      <AboutRow index={1} id="about-problem" title={t("problem.title")}>
        <p className="max-w-[40ch] text-[22px] leading-[1.45] tracking-[-0.01em]">
          {t("problem.big")}
        </p>
        <p className={BODY_CLASS}>{t("problem.body")}</p>
        <ul className="flex flex-wrap gap-1.5">
          {EVENT_KINDS.map((kind) => (
            <li
              key={kind}
              className="border border-dash-rule bg-dash-surface px-2.5 py-1 font-dash-mono text-[12px] text-dash-ink-2"
            >
              {kind}
            </li>
          ))}
        </ul>
      </AboutRow>

      <AboutRow index={2} id="about-approach" title={t("approach.title")}>
        <p className={BODY_CLASS}>{t("approach.body")}</p>
        <div className={`${CELLS_CLASS} wide:grid-cols-3`}>
          {PIPE_KEYS.map((key) => (
            <div key={key} className={`${CELL_CLASS} gap-1.5 px-5 py-[18px]`}>
              <span className={SITE_LABEL_CLASS}>{t(`approach.pipe.${key}.label`)}</span>
              <p className="text-[14px] text-dash-ink-2">{t(`approach.pipe.${key}.body`)}</p>
            </div>
          ))}
        </div>
        <p className={BODY_CLASS}>{t("approach.outro")}</p>
      </AboutRow>

      <AboutRow index={3} id="about-audience" title={t("audience.title")}>
        <div className={`${CELLS_CLASS} wide:grid-cols-3`}>
          {AUDIENCE_KEYS.map((key) => (
            <article key={key} className={`${CELL_CLASS} gap-2.5 px-[22px] py-6`}>
              <span className={SITE_LABEL_CLASS}>{t(`audience.items.${key}.label`)}</span>
              <h3 className="text-[18px] font-semibold">{t(`audience.items.${key}.title`)}</h3>
              <p className="text-[14px] text-dash-ink-2">{t(`audience.items.${key}.body`)}</p>
            </article>
          ))}
        </div>
      </AboutRow>

      <AboutRow index={4} id="about-availability" title={t("availability.title")}>
        <dl className="flex flex-wrap gap-x-8 gap-y-3 border border-dash-rule bg-dash-surface px-5 py-[18px]">
          {TERM_KEYS.map((key) => (
            <div key={key} className="flex flex-col gap-1">
              <dt className={SITE_LABEL_CLASS}>{t(`availability.terms.${key}.label`)}</dt>
              <dd>{t(`availability.terms.${key}.value`)}</dd>
            </div>
          ))}
        </dl>
        <AboutScope />
        <p className={BODY_CLASS}>{t("availability.premium")}</p>
      </AboutRow>

      <AboutRow index={5} id="about-behind" title={t("behind.title")} anchor={BEHIND_ID} last>
        <div className="flex flex-wrap items-center justify-between gap-x-8 gap-y-4 border border-dash-rule bg-dash-surface p-6">
          <div className="flex items-center gap-5">
            <Image
              src="/logo-tampon-sombre.svg"
              alt="AWSD"
              width={72}
              height={72}
              // ⚠️ The optimizer rejects SVG.
              unoptimized
              className="shrink-0"
            />
            <div className="flex flex-col gap-1.5">
              <span className="text-[28px] font-semibold tracking-[-0.02em]">AWSD</span>
              <span className="text-dash-ink-2">{t("behind.tagline")}</span>
            </div>
          </div>
          <div className="flex flex-wrap gap-3">
            <SecondaryButton href={AWSD_URL} label="awsd.fr" external />
            <SecondaryButton href={GITHUB_REPO_URL} label={t("behind.ctaSource")} external />
          </div>
        </div>
      </AboutRow>
    </>
  );
}

function AboutRow({
  index,
  id,
  title,
  anchor,
  last = false,
  children,
}: {
  index: number;
  /** Of the title, for the section's `aria-labelledby`. */
  id: string;
  title: string;
  anchor?: string;
  last?: boolean;
  children: ReactNode;
}) {
  return (
    <section
      id={anchor}
      aria-labelledby={id}
      className={`grid grid-cols-12 gap-x-8 gap-y-4 py-10 rail:py-14 ${last ? "" : "border-b border-dash-rule"}`}
    >
      <div className="col-span-12 flex flex-col gap-2.5 wide:col-span-4">
        <span className="font-dash-mono text-[12px] tracking-[0.08em] text-dash-accent">
          {String(index).padStart(2, "0")}
        </span>
        <h2
          id={id}
          className="text-[26px] leading-[1.1] font-semibold tracking-[-0.025em] rail:text-[32px]"
        >
          {title}
        </h2>
      </div>
      <div className="col-span-12 flex min-w-0 flex-col gap-[18px] wide:col-span-8">
        {children}
      </div>
    </section>
  );
}
