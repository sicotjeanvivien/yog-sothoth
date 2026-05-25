/**
 * Support us page.
 *
 * Three blocks, ordered from lowest-friction to highest-engagement:
 *
 *   1. Header — what this page is.
 *   2. Make it known — star + share (one click, free, visibility).
 *   3. Actions grid — feedback (left) + sponsor (right).
 *
 * At v0.1 the most useful contribution is visibility followed by
 * feedback; sponsoring is offered but not pushed.
 *
 * Pre-deployment review checklist:
 *   - confirm contact email (in support-us-actions.tsx)
 *   - confirm Solana wallet address (in support-us-actions.tsx)
 *   - confirm GitHub Sponsors page is set up at the URL referenced
 *   - confirm site canonical URL (in support-us-make-known.tsx)
 */

import { setRequestLocale, getTranslations } from "next-intl/server";
import type { Metadata } from "next";

import { SupportUsHeader } from "@/components/marketing/support-us/support-us-header";
import { SupportUsMakeKnown } from "@/components/marketing/support-us/support-us-make-known";
import { SupportUsActions } from "@/components/marketing/support-us/support-us-actions";

type SupportUsPageProps = {
  params: Promise<{ locale: string }>;
};

export async function generateMetadata({
  params,
}: SupportUsPageProps): Promise<Metadata> {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: "SupportUs.meta" });
  return {
    title: t("title"),
    description: t("description"),
  };
}

export default async function SupportUsPage({ params }: SupportUsPageProps) {
  const { locale } = await params;
  setRequestLocale(locale);

  return (
    <main>
      <SupportUsHeader />
      <SupportUsMakeKnown />
      <SupportUsActions />
    </main>
  );
}