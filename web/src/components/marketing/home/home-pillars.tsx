/**
 * The homepage's four feature pillars, a Server Component. ⚠️ `id="features"` is the target
 * of the hero's "See features" (`/#features`).
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

type Pillar = {
  key: string;
  icon: FC<IconProps>;
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

export function HomePillars() {
  const t = useTranslations("Marketing.pillars");

  return (
    <section
      id="features"
      className="mx-auto max-w-[1800px] scroll-mt-24 px-6 py-10 lg:px-12"
    >
      <div className="text-center">
        <p className="text-[14px] font-semibold tracking-[0.28em] text-slate-400 uppercase">
          {t("eyebrow")}
        </p>
        <h2 className="mt-4 font-display text-[32px] font-bold tracking-[0.03em] text-[#f5f2ff] lg:text-[40px]">
          {t("title")}
        </h2>
      </div>

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
    <article className="flex gap-4 rounded-md border border-sothoth-500/15 bg-cosmos-900/60 p-5 transition-colors hover:border-sothoth-500/35">
      <div
        className={`flex h-11 w-11 shrink-0 items-center justify-center rounded-3xl border ${accent}`}
      >
        <Icon size={24} />
      </div>

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
