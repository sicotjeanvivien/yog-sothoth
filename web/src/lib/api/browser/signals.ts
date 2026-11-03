/**
 * The signals from the browser, every protocol's or one protocol's, to refill the gap after an
 * SSE reconnection.
 * @throws ApiClientError on any transport, HTTP, or schema failure.
 */

import { apiGetBrowser } from "@/lib/api/client/browser";
import { signalsPath } from "@/lib/signals/signal-paths";

import {
  zListSignalsResponse,
  type PageResponseSignalResponse,
  type ProtocolResponse,
} from "../schema/zod.gen";

const DEFAULT_LIMIT = 50;

export async function fetchSignalsBrowser(
  protocol?: ProtocolResponse,
  limit: number = DEFAULT_LIMIT,
): Promise<PageResponseSignalResponse> {
  return apiGetBrowser(signalsPath(protocol), { limit }, zListSignalsResponse);
}
