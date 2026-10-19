/**
 * The privacy policy. ⚠️ Its copy states Cloudflare dashboard facts:
 * both hosts proxied; Bot Fight Mode, rate limiting, Always Online and Web Analytics off;
 * Challenge Passage at 30 min. Changing one makes the page false. A new
 * `document.cookie` in `web/src` belongs in the cookies section.
 */

import { setRequestLocale, getTranslations } from "next-intl/server";
import type { Metadata } from "next";

import { PrivacyDocument } from "@/components/marketing/privacy/privacy-document";
import { pageMetadata } from "@/lib/seo/page-metadata";

type PrivacyPageProps = {
  params: Promise<{ locale: string }>;
};

export async function generateMetadata({
  params,
}: PrivacyPageProps): Promise<Metadata> {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: "Privacy.meta" });
  return pageMetadata({
    locale,
    path: "/privacy",
    title: t("title"),
    description: t("description"),
  });
}

export default async function PrivacyPage({ params }: PrivacyPageProps) {
  const { locale } = await params;
  setRequestLocale(locale);

  return (
    <main className="mx-auto max-w-[1280px] px-4 rail:px-8">
      <PrivacyDocument />
    </main>
  );
}