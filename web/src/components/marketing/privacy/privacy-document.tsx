/** The privacy policy: one section per processing, since GDPR article 13 asks the same questions of each. */

import { useTranslations } from "next-intl";

import { CONTACT_EMAIL_HREF } from "@/components/marketing/contact";
import { LEGAL_LINK_CLASS, LegalDocument } from "@/components/marketing/legal/legal-document";

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
            {t.rich(`${key}.body`, {
              email: (chunks) => (
                <a href={CONTACT_EMAIL_HREF} className={LEGAL_LINK_CLASS}>
                  {chunks}
                </a>
              ),
            })}
          </p>
        ),
      }))}
    />
  );
}
