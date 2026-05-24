/**
 * About — prose section.
 *
 * Sits directly under the hero. Five stacked cards, each answering
 * one question in plain prose, prefixed with a small icon badge:
 *
 *   1. The problem        — EyeIcon
 *   2. The approach       — PulseIcon
 *   3. Who it's for       — SignalsIcon
 *   4. How it's available — OpenSourceIcon
 *   5. Who is behind it   — UsersIcon
 *
 * Constrained to a comfortable reading width. Copy lives under
 * `About.prose` in `messages/{en,fr}.json`.
 */

import { getTranslations } from "next-intl/server";
import type { FC, ReactNode } from "react";

import {
  EyeIcon,
  OpenSourceIcon,
  PulseIcon,
  SignalsIcon,
  UsersIcon,
  type IconProps,
} from "@/components/shared/icon";

const GITHUB_REPO_URL = "https://github.com/sicotjeanvivien/yog-sothoth";
const AWSD_URL = "https://awsd.fr/";

const INLINE_LINK_CLASS =
  "text-sothoth-400 underline decoration-sothoth-500/40 underline-offset-4 transition-colors hover:text-sothoth-300 hover:decoration-sothoth-400";

const CARD_CLASS =
  "flex gap-5 rounded-[6px] border border-sothoth-500/15 bg-cosmos-900/60 p-6 transition-colors hover:border-sothoth-500/35 lg:p-8";

const ICON_BADGE_CLASS =
  "inline-flex h-[64px] w-[64px] shrink-0 items-center justify-center rounded-[6px] border border-sothoth-500/25 bg-sothoth-600/10 text-sothoth-400";

const TITLE_CLASS =
  "font-display text-[24px] font-semibold tracking-[0.02em] text-[#f1ecff] lg:text-[20px]";

const BODY_CLASS = "mt-3 text-[17px] leading-[1.7] text-slate-300";

// ── Card ordering ─────────────────────────────────────────────────────
//
// The reading flow is intentional: problem → approach → audience →
// access → identity. Reordering breaks the narrative.

type CardConfig = {
  key: string;
  Icon: FC<IconProps>;
};

const CARDS: readonly CardConfig[] = [
  { key: "problem", Icon: EyeIcon },
  { key: "approach", Icon: PulseIcon },
  { key: "audience", Icon: SignalsIcon },
  { key: "availability", Icon: OpenSourceIcon },
  { key: "behind", Icon: UsersIcon },
] as const;

export async function AboutProse() {
  const t = await getTranslations("About.prose");

  return (
    <section className="mx-auto max-w-[1800px] px-6 pb-24 lg:px-12">
      <div className="mx-auto max-w-[128ch] space-y-4">
        {CARDS.map(({ key, Icon }) => (
          <ProseCard key={key} Icon={Icon} title={t(`${key}.title`)}>
            {key === "behind"
              ? t.rich(`${key}.body`, {
                  awsd: (chunks) => (
                    <a
                      href={AWSD_URL}
                      target="_blank"
                      rel="noopener noreferrer"
                      className={INLINE_LINK_CLASS}
                    >
                      {chunks}
                    </a>
                  ),
                  github: (chunks) => (
                    <a
                      href={GITHUB_REPO_URL}
                      target="_blank"
                      rel="noopener noreferrer"
                      className={INLINE_LINK_CLASS}
                    >
                      {chunks}
                    </a>
                  ),
                })
              : t(`${key}.body`)}
          </ProseCard>
        ))}
      </div>
    </section>
  );
}

// ── Sub-component ─────────────────────────────────────────────────────

/**
 * One titled prose card with an icon badge on the left. The badge
 * is fixed-size and aligned to the top of the content so paragraphs
 * of different lengths don't push it around.
 */
function ProseCard({
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
      <div className={ICON_BADGE_CLASS}>
        <Icon size={24} />
      </div>
      <div>
        <h2 className={TITLE_CLASS}>{title}</h2>
        <p className={BODY_CLASS}>{children}</p>
      </div>
    </article>
  );
}