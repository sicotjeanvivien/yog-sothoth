/** The start-up check of the Node server, loaded by `instrumentation.ts`. */

import { loadServerEnv } from "./lib/config/server-env.schema";

try {
  loadServerEnv();
} catch (error) {
  console.error(error instanceof Error ? error.message : error);
  // ⚠️ A throw is not enough: Next logs it, stays up and answers 500 on every page.
  process.exit(1);
}
