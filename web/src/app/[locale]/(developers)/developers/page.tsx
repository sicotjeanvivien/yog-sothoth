/**
 * The API's routes, read from the contract at build, to read and try from the browser. Unlisted:
 * no link leads here, and it is `noindex`.
 */

import type { Metadata } from "next";
import { getTranslations, setRequestLocale } from "next-intl/server";

import { ApiExplorer } from "@/components/developers/api-explorer";
import { UnlistedTag } from "@/components/developers/unlisted-tag";
import { loadClientEnv } from "@/lib/config/client-env.schema";
import { endpointsOf, type OpenApi } from "@/lib/developers/contract";
import { pageMetadata } from "@/lib/seo/page-metadata";

// The contract `gen:api` copies into `public/`, the same file `/openapi.json` serves.
import spec from "../../../../../public/openapi.json";

type DevelopersPageProps = {
  params: Promise<{ locale: string }>;
};

const contract = spec as unknown as OpenApi;

export async function generateMetadata({ params }: DevelopersPageProps): Promise<Metadata> {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: "Developers.page" });
  return {
    ...pageMetadata({
      locale,
      path: "/developers",
      title: t("title"),
      description: t("description"),
    }),
    robots: { index: false, follow: false },
  };
}

export default async function DevelopersPage({ params }: DevelopersPageProps) {
  const { locale } = await params;
  setRequestLocale(locale);
  const t = await getTranslations("Developers");
  const base = loadClientEnv().NEXT_PUBLIC_YOG_API_URL;

  return (
    <>
      <header className="flex flex-wrap items-start justify-between gap-x-8 gap-y-3 border-b border-dash-rule px-4 py-5 rail:px-8 rail:pt-6 rail:pb-[22px]">
        <div className="flex min-w-0 flex-[1_1_360px] flex-wrap items-baseline gap-x-2 gap-y-1.5">
          <h1 className="text-[20px] leading-[1.2] font-semibold tracking-[-0.01em] text-dash-ink">
            {t("page.title")}
          </h1>
          <UnlistedTag />
        </div>
        <div className="flex flex-wrap items-baseline gap-x-5 gap-y-1.5 text-[12px] text-dash-ink-2">
          <span>
            <span className="font-dash-mono text-[11px] font-medium tracking-[0.12em] text-dash-ink-3 uppercase">
              {t("meta.base")}
            </span>{" "}
            <span className="font-dash-mono">{base}</span>
          </span>
          <span>
            <span className="font-dash-mono text-[11px] font-medium tracking-[0.12em] text-dash-ink-3 uppercase">
              {t("meta.spec")}
            </span>{" "}
            <span className="font-dash-mono">
              OpenAPI {contract.openapi.split(".").slice(0, 2).join(".")} · v{contract.info.version}
            </span>
          </span>
          <a href="/openapi.json" download className="font-dash-mono text-dash-accent hover:text-dash-ink">
            {t("meta.download")}
          </a>
        </div>
      </header>
      <ApiExplorer endpoints={endpointsOf(contract)} base={base} />
    </>
  );
}
