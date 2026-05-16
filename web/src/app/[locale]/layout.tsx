import { NextIntlClientProvider, hasLocale } from "next-intl";
import { setRequestLocale } from "next-intl/server";
import { notFound } from "next/navigation";
import { routing } from "../../../i18n/routing";
import "../globals.css";

import { Cinzel } from "next/font/google";

const cinzel = Cinzel({
  subsets: ["latin"],
  weight: ["400", "500", "600"],
  variable: "--font-display",  // exposes a CSS variable
  display: "swap",
});

// Tells Next.js to statically render every supported locale at build time.
// Avoids the dynamic-rendering opt-in that next-intl would otherwise
// trigger as soon as a Server Component reads translations.
export function generateStaticParams() {
  return routing.locales.map((locale) => ({ locale }));
}

type LocaleLayoutProps = {
  children: React.ReactNode;
  // In Next.js 15, `params` is a Promise and must be awaited before use.
  params: Promise<{ locale: string }>;
};

export default async function LocaleLayout({
  children,
  params,
}: LocaleLayoutProps) {
  const { locale } = await params;

  // Defensive validation — the middleware should already have rejected
  // unknown locales, but a direct request to /xx/... could still land here.
  if (!hasLocale(routing.locales, locale)) {
    notFound();
  }

  // Required by next-intl to enable static rendering of pages that
  // consume translations from Server Components.
  setRequestLocale(locale);

  return (
    <html lang={locale} className={cinzel.variable}>
      <body>
        <NextIntlClientProvider>{children}</NextIntlClientProvider>
      </body>
    </html>
  );
}