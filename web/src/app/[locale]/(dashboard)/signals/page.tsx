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
      <header className="px-6 pt-8 pb-6 lg:px-10 lg:pt-10">
        <p className="text-[12px] font-semibold tracking-[0.28em] text-slate-400 uppercase">
          {t("eyebrow")}
        </p>
        <h1 className="mt-2 font-display text-[28px] leading-[1.15] font-bold tracking-[0.03em] text-[#f5f2ff] lg:text-[34px]">
          {t("title")}
        </h1>
        <p className="mt-3 max-w-[68ch] text-[15px] leading-[1.6] text-slate-400">
          {t("description")}
        </p>
      </header>

      <SignalFeed initial={page.items} />
    </>
  );
}
