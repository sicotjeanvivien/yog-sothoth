import { defineConfig, globalIgnores } from "eslint/config";
import nextVitals from "eslint-config-next/core-web-vitals";
import nextTs from "eslint-config-next/typescript";

// Flat config for ESLint 9+. Replaces the legacy `.eslintrc.json`
// format which is being phased out across the ecosystem.
//
// `eslint-config-next/core-web-vitals` upgrades a handful of Next.js
// lint rules from warnings to errors so that issues impacting Core
// Web Vitals fail the lint step rather than silently passing.
//
// `eslint-config-next/typescript` layers TypeScript-specific rules on
// top, mirroring what `create-next-app --typescript` ships with.
export default defineConfig([
  ...nextVitals,
  ...nextTs,
  // Default ignores from eslint-config-next, restated explicitly so
  // overriding them later stays a one-liner change.
  globalIgnores([
    ".next/**",
    "out/**",
    "build/**",
    "next-env.d.ts",
  ]),
]);
