/**
 * The privacy policy, one card per processing (`privacy-prose.tsx`). ⚠️ Its copy states facts
 * held in the Cloudflare dashboard — both hosts proxied; Bot Fight Mode, rate limiting, Always
 * Online, Web Analytics and Email Address Obfuscation off; Challenge Passage at 30 min:
 * changing one makes the page false. A new `document.cookie` in `web/src` belongs on the cookie card.
 * ⚠️ Email Obfuscation rewrites the `mailto:` links here and on /legal-notice, breaking hydration.
 */

import { setRequestLocale, getTranslations } from "next-intl/server";
import type { Metadata } from "next";

import { PrivacyHeader } from "@/components/marketing/privacy/privacy-header";
import { PrivacyProse } from "@/components/marketing/privacy/privacy-prose";

type PrivacyPageProps = {
  params: Promise<{ locale: string }>;
};

export async function generateMetadata({
  params,
}: PrivacyPageProps): Promise<Metadata> {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: "Privacy.meta" });
  return {
    title: t("title"),
    description: t("description"),
  };
}

export default async function PrivacyPage({ params }: PrivacyPageProps) {
  const { locale } = await params;
  setRequestLocale(locale);

  return (
    <main>
      <PrivacyHeader />
      <PrivacyProse />
    </main>
  );
}