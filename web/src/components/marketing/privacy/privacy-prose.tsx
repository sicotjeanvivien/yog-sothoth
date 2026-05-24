/**
 * Privacy — prose section.
 *
 * Six stacked cards, each answering one question in plain prose,
 * prefixed with a small icon badge:
 *
 *   1. In short        — InfoIcon
 *   2. Who is responsible — UserCardIcon
 *   3. What we collect — EyeIcon (reused from About)
 *   4. Cookies         — CookieIcon
 *   5. Your rights     — ShieldIcon
 *   6. Changes         — RefreshIcon
 *
 * Constrained to a comfortable reading width. Copy lives under
 * `Privacy.prose` in `messages/{en,fr}.json`.
 */

import { getTranslations } from "next-intl/server";
import type { FC, ReactNode } from "react";

import {
  CookieIcon,
  EyeIcon,
  InfoIcon,
  RefreshIcon,
  ShieldIcon,
  UserCardIcon,
  type IconProps,
} from "@/components/shared/icon";

const CONTACT_EMAIL_HREF = "mailto:[contact-email]";

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
// The reading flow mirrors what a visitor naturally asks:
// "what's the short version?" → "who is responsible?" →
// "what do you collect?" → "what about cookies?" → "what are my
// rights?" → "what happens if this changes?"

type CardConfig = {
  key: string;
  Icon: FC<IconProps>;
};

const CARDS: readonly CardConfig[] = [
  { key: "inShort", Icon: InfoIcon },
  { key: "responsible", Icon: UserCardIcon },
  { key: "collected", Icon: EyeIcon },
  { key: "cookies", Icon: CookieIcon },
  { key: "rights", Icon: ShieldIcon },
  { key: "changes", Icon: RefreshIcon },
] as const;

export async function PrivacyProse() {
  const t = await getTranslations("Privacy.prose");

  return (
    <section className="mx-auto max-w-[1800px] px-6 pb-24 lg:px-12">
      <div className="mx-auto max-w-[128ch] space-y-4">
        {CARDS.map(({ key, Icon }) => (
          <ProseCard key={key} Icon={Icon} title={t(`${key}.title`)}>
            {/* Cards `responsible` and `rights` need an inline
                mailto link — handled via t.rich. The other cards
                are plain prose. */}
            {key === "responsible" || key === "rights"
              ? t.rich(`${key}.body`, {
                  email: (chunks) => (
                    <a href={CONTACT_EMAIL_HREF} className={INLINE_LINK_CLASS}>
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
 * One titled prose card with an icon badge on the left.
 * Body accepts arbitrary children so callers can pass plain strings
 * or rich-text fragments containing links.
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
        <Icon size={32} />
      </div>
      <div>
        <h2 className={TITLE_CLASS}>{title}</h2>
        <p className={BODY_CLASS}>{children}</p>
      </div>
    </article>
  );
}