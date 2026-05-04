import createMiddleware from "next-intl/middleware";
import { routing } from "../i18n/routing";

// Locale negotiation runs at the network boundary on every matched
// request. Since Next.js 16, this file is named `proxy.ts` and the
// exported function is `proxy` — the rename signals that this layer
// is intended for routing concerns (rewrites, redirects, locale
// detection) rather than application-level middleware logic.
//
// next-intl still exposes the helper under the `next-intl/middleware`
// import path; only the file name and export name are different in
// our project.
//
// Locale resolution priority — as configured by next-intl — is:
//   1. existing prefix in the pathname,
//   2. `NEXT_LOCALE` cookie if set,
//   3. `Accept-Language` header,
//   4. `defaultLocale` from routing.ts.
const intlMiddleware = createMiddleware(routing);

export function proxy(
  request: Parameters<typeof intlMiddleware>[0],
): ReturnType<typeof intlMiddleware> {
  return intlMiddleware(request);
}

export const config = {
  // Skip Next.js internals, API routes and any path that contains a
  // dot (typically static assets like favicon.ico). Everything else
  // is routed through locale negotiation.
  matcher: "/((?!api|_next|_vercel|.*\\..*).*)",
};