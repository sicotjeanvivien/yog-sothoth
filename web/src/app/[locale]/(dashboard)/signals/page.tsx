/**
 * Signals page (`/[locale]/(dashboard)/signals`).
 *
 * Server Component. Fetches the first page of the feed (`GET
 * /api/signals`) for the initial render, then hands over to the
 * `SignalFeed` Client Component, which keeps the list live over SSE.
 *
 * Two display states:
 *   - error → `PageError` (driven by `ApiClientError.details.kind`)
 *   - ok    → header + live feed (which owns its empty state)
 */

import type { Metadata } from "next";
import { getTranslations, setRequestLocale } from "next-intl/server";

import { PageError } from "@/components/dashboard/page-error";
import { InfoPopover } from "@/components/shared/info-popover";
import { SignalFeed } from "@/components/dashboard/signals/signal-feed";
import { ApiClientError } from "@/lib/api/errors";
import { fetchSignals } from "@/lib/api/server/signals";

type SignalsPageProps = {
  params: Promise<{ locale: string }>;
};

export async function generateMetadata({
  params,
}: SignalsPageProps): Promise<Metadata> {
  const { locale } = await params;
  const t = await getTranslations({
    locale,
    namespace: "Dashboard.Signals.page",
  });
  return {
    title: `${t("title")} — Yog-Scope`,
    description: t("description"),
  };
}

export default async function SignalsPage({ params }: SignalsPageProps) {
  const { locale } = await params;
  setRequestLocale(locale);

  const t = await getTranslations("Dashboard.Signals.page");
  const tShell = await getTranslations("Dashboard.shell");

  let page;
  try {
    page = await fetchSignals();
  } catch (err) {
    if (err instanceof ApiClientError) {
      return <PageError kind={err.details.kind} />;
    }
    throw err;
  }

  return (
    <>
      <header className="flex items-center gap-2.5 px-6 pt-6 pb-4 lg:px-10">
        <h1 className="font-display text-[20px] leading-[1.2] font-bold tracking-[0.03em] text-[#f5f2ff]">
          {t("title")}
        </h1>
        <InfoPopover label={tShell("pageInfo")}>{t("description")}</InfoPopover>
      </header>

      <SignalFeed initial={page.items} />
    </>
  );
}
