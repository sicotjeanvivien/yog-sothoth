/**
 * The legal notice, copy under `LegalNotice.cards` in `messages/{en,fr}/legal-notice.json`.
 * The editor section carries the registration number and phone LCEN article 1-1 requires.
 */

import { useTranslations } from "next-intl";
import type { ReactNode } from "react";

import {
  CONTACT_EMAIL_HREF,
  LEGAL_LINK_CLASS,
  LegalDocument,
} from "@/components/marketing/legal/legal-document";

/** Field keys in display order: a new field needs a code and a translation change. */
const FIELD_SECTIONS = [
  {
    key: "editor",
    // No VAT line: AWSD is under the franchise en base de TVA, and the field is omitted.
    fields: ["name", "legalForm", "capital", "registeredOffice", "registration", "phone"],
  },
  { key: "publishingDirector", fields: ["name"] },
  { key: "contact", fields: ["email"] },
  { key: "hosting", fields: ["name", "address", "phone"] },
] as const;

export function LegalNoticeDocument() {
  const t = useTranslations("LegalNotice.cards");

  const sections = [
    ...FIELD_SECTIONS.map(({ key, fields }) => ({
      id: key,
      title: t(`${key}.title`),
      body: (
        <dl className="divide-y divide-dash-rule border-y border-dash-rule">
          {fields.map((field) => (
            <Field key={field} label={t(`${key}.fields.${field}Label`)}>
              {key === "contact" ? (
                <a href={CONTACT_EMAIL_HREF} className={LEGAL_LINK_CLASS}>
                  {t(`${key}.fields.${field}`)}
                </a>
              ) : (
                t(`${key}.fields.${field}`)
              )}
            </Field>
          ))}
        </dl>
      ),
    })),
    {
      id: "intellectualProperty",
      title: t("intellectualProperty.title"),
      body: <p>{t("intellectualProperty.fields.body")}</p>,
    },
  ];

  return <LegalDocument current="legal-notice" sections={sections} />;
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="grid grid-cols-1 gap-1 py-3 text-[15px] rail:grid-cols-[180px_1fr] rail:gap-4">
      <dt className="text-dash-ink-3">{label}</dt>
      <dd className="wrap-break-word text-dash-ink">{children}</dd>
    </div>
  );
}
