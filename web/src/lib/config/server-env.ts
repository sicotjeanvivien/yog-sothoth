/**
 * The server env's entry point: `"server-only"` fails the build on a client import. The schema
 * lives in `server-env.schema.ts`, which vitest can load.
 */

import "server-only";

export {
  loadServerEnv as getServerEnv,
  type ServerEnv,
} from "./server-env.schema";