/** The changelog, from `releases.ts`: one anchored block per release, newest first. */

import { setRequestLocale, getTranslations } from "next-intl/server";
import type { Metadata } from "next";

import { ChangelogHeader } from "@/components/marketing/changelog/changelog-header";
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

  return (
    <main>
      <ChangelogHeader />
      <ChangelogList />
    </main>
  );
}
