import type { NextConfig } from "next";
import createNextIntlPlugin from "next-intl/plugin";

// next-intl plugin — points to the request configuration file used by
// Server Components and pages to load messages for the active locale.
const withNextIntl = createNextIntlPlugin("./i18n/request.ts");

const nextConfig: NextConfig = {
  // Standalone output is required for the minimal Docker image.
  // It bundles only the runtime dependencies actually used by the build,
  // which keeps the production image small.
  output: "standalone",
  reactStrictMode: true,
};

export default withNextIntl(nextConfig);