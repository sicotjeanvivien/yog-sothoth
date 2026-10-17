/** The changelog, from `releases.ts`: one anchored block per release, newest first. */

import { setRequestLocale, getTranslations } from "next-intl/server";
import type { Metadata } from "next";

import { SitePageHeader } from "@/components/marketing/site-page-header";
import { ChangelogList } from "@/components/marketing/changelog/changelog-list";

type ChangelogPageProps = {
  params: Promise<{ locale: string }>;
};

export async function generateMetadata({
  params,
}: ChangelogPageProps): Promise<Metadata> {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: "Changelog.meta" });
  return {
    title: t("title"),
    description: t("description"),
  };
}

export default async function ChangelogPage({ params }: ChangelogPageProps) {
  const { locale } = await params;
  setRequestLocale(locale);
  const t = await getTranslations("Changelog.header");

  return (
    <main className="mx-auto max-w-[1280px] px-4 rail:px-8">
      <SitePageHeader eyebrow={t("eyebrow")} title={t("title")} lead={t("lead")} />
      <ChangelogList />
    </main>
  );
}
