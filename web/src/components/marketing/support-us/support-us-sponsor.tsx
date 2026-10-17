/** The donation in SOL, beside where the money goes and what doesn't change. */

import { useTranslations } from "next-intl";

import { SITE_LABEL_CLASS } from "@/components/marketing/site-label";
import { CopyAddressButton } from "./copy-address-button";

const SOLANA_WALLET_ADDRESS = "3xne74fAwLaYrBkJcuNjUYMFtcJVirEekJP2a27xo2Eq";
const ADDRESS_ID = "sol-address";

const FUND_KEYS = ["infrastructure", "development"] as const;
const PROMISE_KEYS = ["code", "dashboard"] as const;

export function SupportUsSponsor() {
  const t = useTranslations("SupportUs.sponsor");

  return (
    <section
      aria-labelledby="sponsor-title"
      className="mt-8 grid grid-cols-12 border-l border-dash-rule wide:mt-10"
    >
      <div className="col-span-12 flex min-w-0 flex-col gap-4 border-t border-r border-b border-dash-rule bg-dash-surface px-[18px] py-6 rail:px-7 rail:py-8 wide:col-span-7">
        <h2 id="sponsor-title" className="text-[22px] font-semibold tracking-[-0.015em]">
          {t("title")}
        </h2>
        <p className="max-w-[58ch] text-dash-ink-2">{t("body")}</p>
        <div className="mt-2 flex flex-col gap-2.5 border border-dash-rule bg-dash-bg p-[18px]">
          <span className={SITE_LABEL_CLASS}>{t("addressLabel")}</span>
          <div className="flex flex-wrap items-center gap-3">
            <span
              id={ADDRESS_ID}
              className="min-w-0 flex-[1_1_280px] font-dash-mono text-[14px] [overflow-wrap:anywhere]"
            >
              {SOLANA_WALLET_ADDRESS}
            </span>
            <CopyAddressButton
              value={SOLANA_WALLET_ADDRESS}
              label={t("copy")}
              copiedLabel={t("copied")}
              describedBy={ADDRESS_ID}
            />
          </div>
          <span className="text-[12px] text-dash-ink-3">{t("check")}</span>
        </div>
      </div>

      <aside
        aria-label={t("funds.label")}
        className="col-span-12 flex min-w-0 flex-col gap-5 border-r border-b border-dash-rule bg-dash-surface-2 px-[18px] py-6 rail:px-7 rail:py-8 wide:col-span-5 wide:border-t"
      >
        <div>
          <span className={SITE_LABEL_CLASS}>{t("funds.label")}</span>
          <ul className="mt-1">
            {FUND_KEYS.map((key) => (
              <li
                key={key}
                className="flex justify-between gap-4 border-b border-dash-rule py-3"
              >
                <span>{t(`funds.${key}.title`)}</span>
                <span className="text-right text-dash-ink-2">{t(`funds.${key}.body`)}</span>
              </li>
            ))}
          </ul>
        </div>
        <div className="flex flex-col gap-2">
          <span className={`${SITE_LABEL_CLASS} mb-1`}>{t("promise.label")}</span>
          {PROMISE_KEYS.map((key) => (
            <p key={key} className="flex items-baseline gap-2.5">
              <span aria-hidden="true" className="size-2 shrink-0 bg-dash-accent" />
              {t(`promise.${key}`)}
            </p>
          ))}
        </div>
      </aside>
    </section>
  );
}
