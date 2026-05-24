/**
 * Legal notice — content section.
 *
 * Five identification cards rendered as definition lists. Unlike
 * the prose pattern used on About and Privacy, this page is a
 * factual record sheet — each card contains key/value pairs rather
 * than paragraphs. The visual shell (border, padding, icon badge)
 * is identical to the other policy cards for consistency.
 *
 * Card order:
 *
 *   1. Editor — corporate identity of the publisher
 *   2. Publishing director — legal representative
 *   3. Contact — email, optionally phone
 *   4. Hosting — hosting provider's identity
 *   5. Intellectual property — short statement on rights
 *
 * Copy lives under `LegalNotice.cards` in `messages/{en,fr}.json`.
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

const CONTACT_EMAIL_HREF = "mailto:[contact-email]";

const INLINE_LINK_CLASS =
  "text-sothoth-400 underline decoration-sothoth-500/40 underline-offset-4 transition-colors hover:text-sothoth-300 hover:decoration-sothoth-400";

const CARD_CLASS =
  "flex gap-5 rounded-[6px] border border-sothoth-500/15 bg-cosmos-900/60 p-6 transition-colors hover:border-sothoth-500/35 lg:p-8";

const ICON_BADGE_CLASS =
  "inline-flex h-[64px] w-[64px] shrink-0 items-center justify-center rounded-[6px] border border-sothoth-500/25 bg-sothoth-600/10 text-sothoth-400";

const TITLE_CLASS =
  "font-display text-[24px] font-semibold tracking-[0.02em] text-[#f1ecff] lg:text-[20px]";

// ── Card schemas ──────────────────────────────────────────────────────
//
// Each card declares the keys it expects from its i18n bundle. The
// list of keys is held here at the source code level rather than
// in the JSON so adding a new field requires both a code change and
// a translation change — a useful nudge to keep the structure tidy.

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
    fields: [
      "name",
      "legalForm",
      "capital",
      "registeredOffice",
      "siren",
      "vat",
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
    /** Single free-form paragraph rather than k/v pairs. */
    fields: ["body"],
  },
] as const;

// ── Component ─────────────────────────────────────────────────────────

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

// ── Sub-components ────────────────────────────────────────────────────

/**
 * Outer card with icon badge and title. Body is passed in as
 * children so callers can decide between a field list and a
 * free-form paragraph.
 */
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

/**
 * Definition-list wrapper. Renders each child `Field` as a label /
 * value row, with a hairline divider between rows.
 */
function FieldList({ children }: { children: ReactNode }) {
  return (
    <dl className="mt-4 divide-y divide-sothoth-500/10">{children}</dl>
  );
}

/**
 * One row in a field list. Label on the left at a fixed width,
 * value on the right. On narrow screens the row stacks.
 */
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
      <dd className="break-words text-slate-200">{children}</dd>
    </div>
  );
}

/**
 * Free-form prose body — used for the IP statement, which doesn't
 * fit a key/value layout.
 */
function Prose({ children }: { children: ReactNode }) {
  return (
    <p className="mt-3 text-[16px] leading-[1.7] text-slate-300">{children}</p>
  );
}