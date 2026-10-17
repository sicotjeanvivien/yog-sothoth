/**
 * The privacy policy: one section per processing, since GDPR article 13 asks the same
 * questions of each. Copy under `Privacy.prose` in `messages/{en,fr}/privacy.json`.
 */

import { useTranslations } from "next-intl";

import {
  CONTACT_EMAIL_HREF,
  LEGAL_LINK_CLASS,
  LegalDocument,
} from "@/components/marketing/legal/legal-document";

const SECTION_KEYS = [
  "inShort",
  "responsible",
  "browsing",
  "contact",
  "sponsors",
  "thirdParties",
  "cookies",
  "rights",
  "changes",
] as const;

const WITH_EMAIL: ReadonlySet<string> = new Set(["responsible", "rights"]);

export function PrivacyDocument() {
  const t = useTranslations("Privacy.prose");

  return (
    <LegalDocument
      current="privacy"
      sections={SECTION_KEYS.map((key) => ({
        id: key,
        title: t(`${key}.title`),
        brief: key === "inShort",
        body: (
          <p>
            {WITH_EMAIL.has(key)
              ? t.rich(`${key}.body`, {
                  email: (chunks) => (
                    <a href={CONTACT_EMAIL_HREF} className={LEGAL_LINK_CLASS}>
                      {chunks}
                    </a>
                  ),
                })
              : t(`${key}.body`)}
          </p>
        ),
      }))}
    />
  );
}
