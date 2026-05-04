import { getRequestConfig } from "next-intl/server";
import { hasLocale } from "next-intl";
import { routing } from "./routing";

// Server-side configuration consumed by every Server Component that
// reads translations. Next-intl invokes this on each request to learn
// which locale to serve and where to find the matching message bundle.
export default getRequestConfig(async ({ requestLocale }) => {
  // The locale here comes from the `[locale]` segment in the URL,
  // already validated by the middleware. We still defensively narrow
  // it to a known locale and fall back to the default otherwise.
  const requested = await requestLocale;
  const locale = hasLocale(routing.locales, requested)
    ? requested
    : routing.defaultLocale;

  return {
    locale,
    messages: (await import(`../messages/${locale}.json`)).default,
  };
});