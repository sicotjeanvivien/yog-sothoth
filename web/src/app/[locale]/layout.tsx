import { NextIntlClientProvider, hasLocale } from "next-intl";
import { setRequestLocale } from "next-intl/server";
import { notFound } from "next/navigation";
import { routing } from "../../../i18n/routing";
import "../globals.css";

import { Cinzel, Geist, Geist_Mono } from "next/font/google";

const cinzel = Cinzel({
  subsets: ["latin"],
  weight: ["400", "500", "600"],
  variable: "--font-display",  // exposes a CSS variable
  display: "swap",
});
// Read by the `--font-dash-*` theme tokens, on the site and the dashboard alike.
const geist = Geist({
  subsets: ["latin"],
  variable: "--font-geist",
  display: "swap",
});
const geistMono = Geist_Mono({
  subsets: ["latin"],
  variable: "--font-geist-mono",
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
    <html
      lang={locale}
      className={`${cinzel.variable} ${geist.variable} ${geistMono.variable}`}
    >
      <body>
        <NextIntlClientProvider>{children}</NextIntlClientProvider>
      </body>
    </html>
  );
}