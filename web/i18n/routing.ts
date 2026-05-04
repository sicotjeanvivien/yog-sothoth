import { defineRouting } from "next-intl/routing";

// Central routing configuration shared by the middleware and the
// navigation API wrappers. The `as const` cast on `locales` lets
// TypeScript narrow the locale type everywhere it is consumed.
export const routing = defineRouting({
  locales: ["en", "fr"] as const,
  defaultLocale: "en",
  // `always` keeps the locale prefix visible for every route, including
  // the default one. Chosen on the project side to avoid ambiguity in
  // logs, share links and analytics.
  localePrefix: "always",
});

export type Locale = (typeof routing.locales)[number];