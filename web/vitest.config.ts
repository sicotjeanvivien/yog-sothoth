import { defineConfig } from "vitest/config";
import { resolve } from "node:path";

// Vitest configuration for the /web package.
//
// The `@/*` alias mirrors the one declared in tsconfig.json so that
// test files can use the same import paths as the application code
// (e.g. `import { ... } from "@/config/features"`).
export default defineConfig({
  test: {
    // Plain Node environment is enough for unit tests on pure logic.
    // The day we add component tests that touch the DOM, switch this
    // to "jsdom" or "happy-dom" (per-file overrides are also possible).
    environment: "node",
    // Default include pattern is broad enough to pick up tests both in
    // co-located __tests__ folders and in adjacent *.test.ts files.
    include: ["src/**/*.{test,spec}.{ts,tsx}"],
    globals: false,
    // CI-friendly defaults: fail fast and emit a JUnit-style summary
    // line on completion. Tweak when local DX needs differ.
    reporters: ["default"],
  },
  resolve: {
    alias: {
      "@": resolve(__dirname, "src"),
    },
  },
});