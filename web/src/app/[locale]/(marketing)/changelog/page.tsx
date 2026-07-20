/**
 * Changelog page.
 *
 * Static, data-driven release history (`releases.ts` — no MDX, no
 * dependency): one anchored block per release, newest first. Release
 * announcements published in the `announcements` table point here via
 * `link_url` (`/changelog#vX.Y.Z`).
 */

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
