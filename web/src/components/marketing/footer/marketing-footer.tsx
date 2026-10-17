/**
 * The marketing footer, a Server Component: brand, the `FOOTER_COLUMNS` link columns, the
 * AWSD credit, then the copyright, the version and the language.
 */

import Image from "next/image";
import { useTranslations } from "next-intl";

import { Link } from "@/i18n/navigation";
import { SiteBrand } from "@/components/marketing/site-brand";
import { RELEASES } from "@/components/marketing/changelog/releases";
import { FooterLocaleSwitch } from "./footer-locale-switch";
import {
  FOOTER_AWSD,
  FOOTER_COLUMNS,
  FOOTER_SOCIALS,
  type FooterColumn,
} from "./marketing-footer-links";

const HEADING_CLASS =
  "font-dash-mono text-[11px] font-medium tracking-[0.12em] text-dash-ink-3 uppercase";

export function MarketingFooter() {
  const t = useTranslations("Marketing.footer");
  const year = new Date().getFullYear();

  return (
    <footer className="pt-14 pb-8">
      <div className="mx-auto max-w-[1280px] px-4 rail:px-8">
        <div className="grid grid-cols-12 gap-8">
          <BrandBlock />
          {FOOTER_COLUMNS.map((column) => (
            <LinksColumn key={column.key} column={column} />
          ))}
          <AwsdColumn />
        </div>

        <div className="mt-12 flex flex-wrap items-center justify-between gap-x-6 gap-y-2 border-t border-dash-rule pt-5 text-[12px] text-dash-ink-3">
          <span>{t("copyright", { year })}</span>
          <span className="inline-flex items-center gap-4">
            <span className="font-dash-mono">{RELEASES[0]?.version}</span>
            <FooterLocaleSwitch label={t("language")} />
          </span>
        </div>
      </div>
    </footer>
  );
}

function BrandBlock() {
  const t = useTranslations("Marketing.footer");

  return (
    <div className="col-span-12 flex flex-col gap-4 wide:col-span-5">
      <div>
        <SiteBrand />
      </div>
      <p className="max-w-[36ch] text-dash-ink-2">{t("description")}</p>
      <div className="flex gap-2">
        {FOOTER_SOCIALS.map((social) => (
          <a
            key={social.key}
            href={social.href}
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex min-h-11 min-w-11 items-center justify-center border border-dash-rule px-3 font-dash-mono text-[12px] text-dash-ink-2 transition-colors hover:border-dash-ink-3 hover:text-dash-ink"
          >
            {social.label}
          </a>
        ))}
      </div>
    </div>
  );
}

function LinksColumn({ column }: { column: FooterColumn }) {
  const t = useTranslations(column.namespace);

  return (
    <nav
      aria-label={t("heading")}
      className="col-span-12 flex flex-col gap-1 rail:col-span-4 wide:col-span-2"
    >
      <h3 className={HEADING_CLASS}>{t("heading")}</h3>
      <ul className="mt-2 flex flex-col">
        {column.links.map((link) => (
          <li key={link.key}>
            <Link
              href={link.href}
              className="inline-flex min-h-11 items-center text-[14px] text-dash-ink-2 transition-colors hover:text-dash-ink rail:min-h-8"
            >
              {t(link.labelKey)}
            </Link>
          </li>
        ))}
      </ul>
    </nav>
  );
}

function AwsdColumn() {
  const t = useTranslations("Marketing.footer.awsd");

  return (
    <div className="col-span-12 flex flex-col gap-1 rail:col-span-4 wide:col-span-3">
      <h3 className={HEADING_CLASS}>{t("title")}</h3>
      <div className="mt-2.5 flex flex-col items-start gap-2">
        <a
          href={FOOTER_AWSD.href}
          target="_blank"
          rel="noopener noreferrer"
          aria-label={t("label")}
          className="inline-flex min-h-11 items-center"
        >
          <Image
            src="/logo-horizontal-sombre.svg"
            alt=""
            width={105}
            height={36}
            unoptimized
          />
        </a>
        <p className="text-[14px] text-dash-ink-2">{t("tagline")}</p>
      </div>
    </div>
  );
}
