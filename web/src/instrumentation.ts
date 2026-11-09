/** Run once when the server starts: a missing or malformed server variable stops it there. */

export async function register(): Promise<void> {
  // ⚠️ Imported only here: Next also bundles this file for the Edge, where `process.exit` is not.
  if (process.env.NEXT_RUNTIME === "nodejs") await import("./instrumentation-node");
}
