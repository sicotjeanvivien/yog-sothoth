/**
 * The security headers every dashboard response carries, applied by
 * `headers()` in `next.config.ts`.
 *
 * Only the headers that cost a read-only application nothing. The dashboard
 * has no account, no form and no action; what these close is framing,
 * MIME sniffing, the full URL leaking to other sites, and plugin content.
 *
 * What the CSP leaves out, on purpose:
 * - **no `script-src` / `default-src`.** Next.js inlines its own bootstrap
 *   scripts, so a script policy that holds needs a per-request nonce — which
 *   means rendering every page through `src/proxy.ts` and giving up the cache
 *   of pre-rendered pages. That trade is still open.
 * - **no `img-src`.** Token logos come from whatever host each token registry
 *   names (`pool-pair-cell.tsx` renders `logoUri` in a native `<img>`).
 *   Restricting it would break the logos without closing the IP leak to those
 *   hosts, which is a privacy question, not a CSP one.
 *
 * HSTS is not here: it belongs to the TLS edge, `docker/Caddyfile`.
 */

export type SecurityHeader = { key: string; value: string };

/**
 * The headers for a dashboard whose browser code calls the API at `apiUrl`
 * (`NEXT_PUBLIC_YOG_API_URL`). The browser reaches the API directly — `fetch`
 * and the signal feed's `EventSource` — so its origin is the one
 * `connect-src` admits besides the dashboard's own.
 */
export function securityHeaders(apiUrl: string): SecurityHeader[] {
  const csp = [
    "frame-ancestors 'none'",
    "object-src 'none'",
    "base-uri 'self'",
    `connect-src 'self' ${new URL(apiUrl).origin}`,
  ].join("; ");

  return [
    { key: "X-Content-Type-Options", value: "nosniff" },
    { key: "Referrer-Policy", value: "strict-origin-when-cross-origin" },
    { key: "Content-Security-Policy", value: csp },
  ];
}
