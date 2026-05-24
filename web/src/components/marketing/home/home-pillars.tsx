/**
 * Homepage feature-pillars section.
 *
 * Four cards — Observe, Reconstruct, Analyze, Signal — in a
 * responsive grid below a short section heading.
 *
 * Card layout: two columns. The icon sits in its own square badge on
 * the LEFT; the title and description stack on the RIGHT. Each card
 * carries its own accent colour for the icon badge.
 *
 * Cards are purely descriptive — no "learn more" link.
 *
 * Static content, Server Component.
 *
 * # Anchor
 *
 * The section carries `id="features"`, so the hero's "See features"
 * button (href `/#features`) scrolls here.
 */

import { useTranslations } from "next-intl";
import type { FC } from "react";

import {
  EyeIcon,
  PoolsIcon,
  PulseIcon,
  SignalsIcon,
  type IconProps,
} from "@/components/shared/icon";

// ── Pillar configuration ──────────────────────────────────────────────
//
// Ordered list of pillars. Each entry pairs a key (resolved against
// the `Marketing.pillars.items` i18n namespace), an icon, and an
// accent — the icon-badge colour. `accent` holds the Tailwind classes
// (text colour + translucent fill + border tint) as a ready string,
// so the card component stays a plain consumer.

type Pillar = {
  key: string;
  icon: FC<IconProps>;
  /** Icon-badge classes — text colour + translucent fill + border. */
  accent: string;
};

const PILLARS: readonly Pillar[] = [
  {
    key: "observe",
    icon: EyeIcon,
    accent: "text-sothoth-400 bg-sothoth-600/15 border-sothoth-500/25",
  },
  {
    key: "reconstruct",
    icon: PoolsIcon,
    accent: "text-eldritch-400 bg-eldritch-500/15 border-eldritch-500/25",
  },
  {
    key: "analyze",
    icon: PulseIcon,
    accent: "text-signal-good bg-signal-good/15 border-signal-good/25",
  },
  {
    key: "signal",
    icon: SignalsIcon,
    accent: "text-signal-bad bg-signal-bad/15 border-signal-bad/25",
  },
] as const;

// ── Section ───────────────────────────────────────────────────────────

export function HomePillars() {
  const t = useTranslations("Marketing.pillars");

  return (
    <section
      id="features"
      className="mx-auto max-w-[1800px] scroll-mt-24 px-6 py-10 lg:px-12"
    >
      {/* Section heading */}
      <div className="text-center">
        <p className="text-[14px] font-semibold tracking-[0.28em] text-slate-400 uppercase">
          {t("eyebrow")}
        </p>
        <h2 className="mt-4 font-display text-[32px] font-bold tracking-[0.03em] text-[#f5f2ff] lg:text-[40px]">
          {t("title")}
        </h2>
      </div>

      {/* Cards grid */}
      <div className="mt-12 grid grid-cols-1 gap-5 sm:grid-cols-2 lg:grid-cols-4">
        {PILLARS.map((pillar) => (
          <PillarCard
            key={pillar.key}
            pillarKey={pillar.key}
            Icon={pillar.icon}
            accent={pillar.accent}
          />
        ))}
      </div>
    </section>
  );
}

// ── Card ──────────────────────────────────────────────────────────────

/**
 * A single pillar card — two columns.
 *
 *   ┌──────┬─────────────────────┐
 *   │ icon │ title               │
 *   │ badge│ description …       │
 *   └──────┴─────────────────────┘
 *
 * `flex` row: the icon badge keeps its size (`shrink-0`), the text
 * column takes the rest (`min-w-0` so long words wrap instead of
 * overflowing).
 */
function PillarCard({
  pillarKey,
  Icon,
  accent,
}: {
  pillarKey: string;
  Icon: FC<IconProps>;
  accent: string;
}) {
  const t = useTranslations("Marketing.pillars.items");

  return (
    <article className="flex gap-4 rounded-[6px] border border-sothoth-500/15 bg-cosmos-900/60 p-5 transition-colors hover:border-sothoth-500/35">
      {/* Left column — icon badge, per-card accent colour */}
      <div
        className={`flex h-11 w-11 shrink-0 items-center justify-center rounded-[24px] border ${accent}`}
      >
        <Icon size={24} />
      </div>

      {/* Right column — title + description */}
      <div className="min-w-0">
        <h3 className="font-display text-[24px] font-semibold tracking-[0.04em] text-[#f1ecff]">
          {t(`${pillarKey}.title`)}
        </h3>
        <p className="mt-1.5 text-[17px] leading-[1.6] text-slate-400">
          {t(`${pillarKey}.body`)}
        </p>
      </div>
    </article>
  );
}