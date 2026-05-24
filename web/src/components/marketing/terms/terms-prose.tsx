/**
 * Terms — prose section.
 *
 * Eight stacked cards covering, in order: a short summary, the
 * service, who can use it, acceptable use, the no-financial-advice
 * disclaimer, availability and accuracy limits, intellectual
 * property, and governing law.
 *
 * The "no financial advice" card uses a distinct `warning` variant
 * — amber accents instead of the default violet — to make the
 * legally critical disclaimer visually unmissable. The shell is
 * otherwise identical to the other cards.
 *
 * Copy lives under `Terms.prose` in `messages/{en,fr}/terms.json`.
 */

import { getTranslations } from "next-intl/server";
import type { FC, ReactNode } from "react";

import {
  AlertTriangleIcon,
  EyeIcon,
  InfoIcon,
  OpenSourceIcon,
  PulseIcon,
  RefreshIcon,
  ShieldIcon,
  UsersIcon,
  type IconProps,
} from "@/components/shared/icon";

// ── Style tokens ─────────────────────────────────────────────────────

type Variant = "default" | "warning";

const CARD_CLASS: Record<Variant, string> = {
  default:
    "flex gap-5 rounded-[6px] border border-sothoth-500/15 bg-cosmos-900/60 p-6 transition-colors hover:border-sothoth-500/35 lg:p-8",
  warning:
    "flex gap-5 rounded-[6px] border border-amber-500/40 bg-amber-950/20 p-6 transition-colors hover:border-amber-500/60 lg:p-8",
};

const ICON_BADGE_CLASS: Record<Variant, string> = {
  default:
    "inline-flex h-[64px] w-[64px] shrink-0 items-center justify-center rounded-[6px] border border-sothoth-500/25 bg-sothoth-600/10 text-sothoth-400",
  warning:
    "inline-flex h-[64px] w-[64px] shrink-0 items-center justify-center rounded-[6px] border border-amber-500/40 bg-amber-500/15 text-amber-400",
};

const TITLE_CLASS: Record<Variant, string> = {
  default:
    "font-display text-[24px] font-semibold tracking-[0.02em] text-[#f1ecff] lg:text-[20px]",
  warning:
    "font-display text-[24px] font-semibold tracking-[0.02em] text-amber-200 lg:text-[20px]",
};

const BODY_CLASS: Record<Variant, string> = {
  default: "mt-3 text-[17px] leading-[1.7] text-slate-300",
  warning: "mt-3 text-[17px] leading-[1.7] text-amber-100/90",
};

// ── Card configuration ────────────────────────────────────────────────
//
// The reading flow is intentional: summary → what is the service →
// who can use it → how → critical disclaimer → service-level
// disclaimer → IP → legal framework. Reordering breaks the narrative.

type CardConfig = {
  key: string;
  Icon: FC<IconProps>;
  variant: Variant;
};

const CARDS: readonly CardConfig[] = [
  { key: "inShort", Icon: InfoIcon, variant: "default" },
  { key: "service", Icon: EyeIcon, variant: "default" },
  { key: "audience", Icon: UsersIcon, variant: "default" },
  { key: "acceptableUse", Icon: ShieldIcon, variant: "default" },
  { key: "financialAdvice", Icon: AlertTriangleIcon, variant: "warning" },
  { key: "availability", Icon: PulseIcon, variant: "default" },
  { key: "intellectualProperty", Icon: OpenSourceIcon, variant: "default" },
  { key: "governingLaw", Icon: RefreshIcon, variant: "default" },
] as const;

// ── Component ─────────────────────────────────────────────────────────

export async function TermsProse() {
  const t = await getTranslations("Terms.prose");

  return (
    <section className="mx-auto max-w-[1800px] px-6 pb-24 lg:px-12">
      <div className="mx-auto max-w-[128ch] space-y-4">
        {CARDS.map(({ key, Icon, variant }) => (
          <ProseCard
            key={key}
            Icon={Icon}
            title={t(`${key}.title`)}
            variant={variant}
          >
            {t(`${key}.body`)}
          </ProseCard>
        ))}
      </div>
    </section>
  );
}

// ── Sub-component ─────────────────────────────────────────────────────

/**
 * One titled prose card. The `variant` prop toggles between the
 * default violet styling and an amber `warning` style — body and
 * icon are otherwise identical.
 */
function ProseCard({
  Icon,
  title,
  variant,
  children,
}: {
  Icon: FC<IconProps>;
  title: string;
  variant: Variant;
  children: ReactNode;
}) {
  return (
    <article className={CARD_CLASS[variant]}>
      <div className={ICON_BADGE_CLASS[variant]}>
        <Icon size={32} />
      </div>
      <div>
        <h2 className={TITLE_CLASS[variant]}>{title}</h2>
        <p className={BODY_CLASS[variant]}>{children}</p>
      </div>
    </article>
  );
}