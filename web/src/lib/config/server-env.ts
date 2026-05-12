/**
 * Server-only entry point for the validated environment configuration.
 *
 * The `"server-only"` directive guarantees that any attempt to import
 * this module from a Client Component fails the Next.js build with a
 * clear error. The actual schema and parsing logic live in the sibling
 * `server-env.schema.ts` module, which is bundler-neutral and exercised
 * directly by the vitest suite.
 */

import "server-only";

export {
  loadServerEnv as getServerEnv,
  type ServerEnv,
} from "./server-env.schema";