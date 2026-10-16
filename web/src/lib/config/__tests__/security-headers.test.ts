/**
 * The security headers, written out whole: a dropped or loosened directive must fail here.
 */

import { describe, expect, it } from "vitest";
import { securityHeaders } from "../security-headers";

function asRecord(apiUrl: string): Record<string, string> {
  return Object.fromEntries(securityHeaders(apiUrl).map((h) => [h.key, h.value]));
}

describe("securityHeaders", () => {
  it("sets exactly nosniff, the referrer policy and the CSP", () => {
    expect(asRecord("https://api.yog-scope.xyz")).toEqual({
      "X-Content-Type-Options": "nosniff",
      "Referrer-Policy": "strict-origin-when-cross-origin",
      "Content-Security-Policy":
        "frame-ancestors 'none'; object-src 'none'; base-uri 'self'; " +
        "connect-src 'self' https://api.yog-scope.xyz",
    });
  });

  it("admits the API's origin in connect-src, not its full URL", () => {
    const csp = asRecord("http://localhost:5000/some/prefix")["Content-Security-Policy"];

    expect(csp?.endsWith("connect-src 'self' http://localhost:5000")).toBe(true);
    expect(csp).not.toContain("/some/prefix");
  });

  it("throws on an API URL that is not a URL", () => {
    expect(() => securityHeaders("api.yog-scope.xyz")).toThrow();
  });
});
