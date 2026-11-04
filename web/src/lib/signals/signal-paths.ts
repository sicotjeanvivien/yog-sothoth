/**
 * Where the signals are read, every protocol's or one protocol's: the page a list starts from
 * and refills from; the live stream that follows it is `livePath`'s.
 */

import type { ProtocolResponse } from "@/lib/api/schema/zod.gen";

/** The collection: `/api/signals`, or `/api/protocols/{protocol}/signals`. */
export function signalsPath(protocol?: ProtocolResponse): string {
  return protocol === undefined ? "/api/signals" : `/api/protocols/${protocol}/signals`;
}
