import type { Metadata } from "next";
import { NextIntlClientProvider, hasLocale } from "next-intl";
import { setRequestLocale } from "next-intl/server";
import { notFound } from "next/navigation";
import { routing } from "../../../i18n/routing";
import { SITE_NAME, SITE_URL } from "@/config/site";
import "../globals.css";

import localFont from "next/font/local";
import "../fonts/fonts.css";

// ⚠️ Google's own files, kept here: a build fetching them fails when Google is down. Each family
// bears its const's name, the one Turbopack gives the CSS variable; `fonts.css` adds the subsets.
const cinzel = localFont({
  src: "../fonts/cinzel-latin.woff2",
  weight: "400 600",
  declarations: [
    { prop: "font-family", value: "cinzel" },
    { prop: "unicode-range", value: "U+0000-00FF, U+0131, U+0152-0153, U+02BB-02BC, U+02C6, U+02DA, U+02DC, U+0304, U+0308, U+0329, U+2000-206F, U+20AC, U+2122, U+2191, U+2193, U+2212, U+2215, U+FEFF, U+FFFD" },
  ],
  variable: "--font-display",  // exposes a CSS variable
  display: "swap",
});
// Read by the `--font-dash-*` theme tokens, on the site and the dashboard alike.
const geist = localFont({
  src: "../fonts/geist-latin.woff2",
  weight: "100 900",
  declarations: [
    { prop: "font-family", value: "geist" },
    { prop: "unicode-range", value: "U+0000-00FF, U+0131, U+0152-0153, U+02BB-02BC, U+02C6, U+02DA, U+02DC, U+0304, U+0308, U+0329, U+2000-206F, U+20AC, U+2122, U+2191, U+2193, U+2212, U+2215, U+FEFF, U+FFFD" },
  ],
  variable: "--font-geist",
  display: "swap",
});
const geistMono = localFont({
  src: "../fonts/geist-mono-latin.woff2",
  weight: "100 900",
  declarations: [
    { prop: "font-family", value: "geistMono" },
    { prop: "unicode-range", value: "U+0000-00FF, U+0131, U+0152-0153, U+02BB-02BC, U+02C6, U+02DA, U+02DC, U+0304, U+0308, U+0329, U+2000-206F, U+20AC, U+2122, U+2191, U+2193, U+2212, U+2215, U+FEFF, U+FFFD" },
  ],
  variable: "--font-geist-mono",
  display: "swap",
});

// Every locale rendered statically, or next-intl's translations opt into dynamic rendering.
export function generateStaticParams() {
  return routing.locales.map((locale) => ({ locale }));
}

/** The site's frame: absolute URLs, and the one place the title suffix is written. */
export const metadata: Metadata = {
  metadataBase: new URL(SITE_URL),
  title: { template: `%s — ${SITE_NAME}`, default: SITE_NAME },
  manifest: "/site.webmanifest",
};

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