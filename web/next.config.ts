import type { NextConfig } from "next";
import createNextIntlPlugin from "next-intl/plugin";

import { loadClientEnv } from "./src/lib/config/client-env.schema";
import { securityHeaders } from "./src/lib/config/security-headers";

// next-intl plugin — points to the request configuration file used by
// Server Components and pages to load messages for the active locale.
const withNextIntl = createNextIntlPlugin("./i18n/request.ts");

const nextConfig: NextConfig = {
  // Standalone output is required for the minimal Docker image.
  // It bundles only the runtime dependencies actually used by the build,
  // which keeps the production image small.
  output: "standalone",
  reactStrictMode: true,
  // Don't announce the framework (`X-Powered-By: Next.js`).
  poweredByHeader: false,
  // Resolved at build time, like every `NEXT_PUBLIC_*` value: the CSP names
  // the API origin the client bundle was built against. `loadClientEnv`
  // validates it, so a missing URL stops the build instead of shipping a CSP
  // that silently blocks the API.
  async headers() {
    return [
      {
        source: "/:path*",
        headers: securityHeaders(loadClientEnv().NEXT_PUBLIC_YOG_API_URL),
      },
    ];
  },
};

export default withNextIntl(nextConfig);
