/**
 * The legal notice: five identification cards as definition lists, copy under
 * `LegalNotice.cards` in `messages/{en,fr}/legal-notice.json`. The editor card carries the registration
 * number and phone LCEN article 1-1 requires.
 */

import { getTranslations } from "next-intl/server";
import type { FC, ReactNode } from "react";

import {
  EditIcon,
  OpenSourceIcon,
  PulseIcon,
  ServerIcon,
  UserCardIcon,
  type IconProps,
} from "@/components/shared/icon";

const CONTACT_EMAIL_HREF = "mailto:awsd.contact@gmail.com";

const INLINE_LINK_CLASS =
  "text-sothoth-400 underline decoration-sothoth-500/40 underline-offset-4 transition-colors hover:text-sothoth-300 hover:decoration-sothoth-400";

const CARD_CLASS =
  "flex gap-5 rounded-[6px] border border-sothoth-500/15 bg-cosmos-900/60 p-6 transition-colors hover:border-sothoth-500/35 lg:p-8";

const ICON_BADGE_CLASS =
  "inline-flex h-[64px] w-[64px] shrink-0 items-center justify-center rounded-[6px] border border-sothoth-500/25 bg-sothoth-600/10 text-sothoth-400";

const TITLE_CLASS =
  "font-display text-[24px] font-semibold tracking-[0.02em] text-[#f1ecff] lg:text-[20px]";

// Each card lists its field keys here, so a new field needs a code and a translation change.

type FieldKey = string;

type CardSchema = {
  /** i18n key under `LegalNotice.cards` */
  key: string;
  Icon: FC<IconProps>;
  /** Field keys, in display order. */
  fields: readonly FieldKey[];
};

const CARDS: readonly CardSchema[] = [
  {
    key: "editor",
    Icon: UserCardIcon,
    // No VAT line: AWSD is under the franchise en base de TVA, and the field is omitted.
    fields: [
      "name",
      "legalForm",
      "capital",
      "registeredOffice",
      "registration",
      "phone",
    ],
  },
  {
    key: "publishingDirector",
    Icon: EditIcon,
    fields: ["name"],
  },
  {
    key: "contact",
    Icon: PulseIcon,
    fields: ["email"],
  },
  {
    key: "hosting",
    Icon: ServerIcon,
    fields: ["name", "address", "phone"],
  },
  {
    key: "intellectualProperty",
    Icon: OpenSourceIcon,
    fields: ["body"],
  },
] as const;

export async function LegalNoticeContent() {
  const t = await getTranslations("LegalNotice.cards");

  return (
    <section className="mx-auto max-w-[1800px] px-6 pb-24 lg:px-12">
      <div className="mx-auto max-w-[128ch] space-y-4">
        {CARDS.map((card) => (
          <Card
            key={card.key}
            Icon={card.Icon}
            title={t(`${card.key}.title`)}
          >
            {card.key === "intellectualProperty" ? (
              <Prose>{t(`${card.key}.fields.body`)}</Prose>
            ) : card.key === "contact" ? (
              <FieldList>
                <Field label={t(`${card.key}.fields.emailLabel`)}>
                  <a href={CONTACT_EMAIL_HREF} className={INLINE_LINK_CLASS}>
                    {t(`${card.key}.fields.email`)}
                  </a>
                </Field>
              </FieldList>
            ) : (
              <FieldList>
                {card.fields.map((field) => (
                  <Field
                    key={field}
                    label={t(`${card.key}.fields.${field}Label`)}
                  >
                    {t(`${card.key}.fields.${field}`)}
                  </Field>
                ))}
              </FieldList>
            )}
          </Card>
        ))}
      </div>
    </section>
  );
}

function Card({
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
      <div className="min-w-0 flex-1">
        <h2 className={TITLE_CLASS}>{title}</h2>
        {children}
      </div>
    </article>
  );
}

function FieldList({ children }: { children: ReactNode }) {
  return (
    <dl className="mt-4 divide-y divide-sothoth-500/10">{children}</dl>
  );
}

function Field({
  label,
  children,
}: {
  label: string;
  children: ReactNode;
}) {
  return (
    <div className="grid grid-cols-1 gap-1 py-3 text-[14px] sm:grid-cols-[160px_1fr] sm:gap-4">
      <dt className="text-slate-500">{label}</dt>
      <dd className="wrap-break-word text-slate-200">{children}</dd>
    </div>
  );
}

function Prose({ children }: { children: ReactNode }) {
  return (
    <p className="mt-3 text-[16px] leading-[1.7] text-slate-300">{children}</p>
  );
}