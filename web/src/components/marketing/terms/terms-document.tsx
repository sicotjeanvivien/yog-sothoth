/** The terms of service. Copy under `Terms.prose` in `messages/{en,fr}/terms.json`. */

import { useTranslations } from "next-intl";

import { LegalDocument } from "@/components/marketing/legal/legal-document";

const SECTION_KEYS = [
  "inShort",
  "service",
  "audience",
  "acceptableUse",
  "financialAdvice",
  "availability",
  "intellectualProperty",
  "governingLaw",
] as const;

export function TermsDocument() {
  const t = useTranslations("Terms.prose");

  return (
    <LegalDocument
      current="terms"
      sections={SECTION_KEYS.map((key) => ({
        id: key,
        title: t(`${key}.title`),
        brief: key === "inShort",
        body: <p>{t(`${key}.body`)}</p>,
      }))}
    />
  );
}
