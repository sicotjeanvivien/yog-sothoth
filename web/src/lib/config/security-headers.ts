/**
 * The security headers of every dashboard response (`next.config.ts`). The CSP leaves out
 * `script-src`, which would need a per-request nonce and lose the pre-rendered cache, and
 * `img-src`, since token logos come from any host. HSTS belongs to the server's entry proxy.
 */

export type SecurityHeader = { key: string; value: string };

/** `apiUrl` is the one origin `connect-src` admits besides the dashboard's: the browser's
 *  `fetch` and `EventSource` reach the API directly. */
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
