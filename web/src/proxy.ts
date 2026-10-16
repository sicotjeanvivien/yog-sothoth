import createMiddleware from "next-intl/middleware";
import { routing } from "../i18n/routing";

// Locale negotiation, on every matched request: the path prefix, then the `NEXT_LOCALE`
// cookie, then `Accept-Language`, then `defaultLocale`. Next.js 16 names this file `proxy.ts`.
const intlMiddleware = createMiddleware(routing);

export function proxy(
  request: Parameters<typeof intlMiddleware>[0],
): ReturnType<typeof intlMiddleware> {
  return intlMiddleware(request);
}

export const config = {
  // Not Next.js internals, API routes, or a path with a dot (static assets).
  matcher: "/((?!api|_next|_vercel|.*\\..*).*)",
};