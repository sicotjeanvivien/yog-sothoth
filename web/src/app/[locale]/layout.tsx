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

// Every locale rendered statically, or next-intl's translations opt into dynamic rendering.
export function generateStaticParams() {
  return routing.locales.map((locale) => ({ locale }));
}

type LocaleLayoutProps = {
  children: React.ReactNode;
  params: Promise<{ locale: string }>;
};

export default async function LocaleLayout({
  children,
  params,
}: LocaleLayoutProps) {
  const { locale } = await params;

  // The proxy rejects unknown locales, but a direct request can still land here.
  if (!hasLocale(routing.locales, locale)) {
    notFound();
  }

  // next-intl needs it for static rendering of translated Server Components.
  setRequestLocale(locale);

  return (
    <html lang={locale} className={cinzel.variable}>
      <body>
        <NextIntlClientProvider>{children}</NextIntlClientProvider>
      </body>
    </html>
  );
}