/**
 * `GET /api/protocols/{protocol}/signals` on the server: one page of the protocol's signals,
 * newest first.
 * @throws ApiClientError on any transport, HTTP, or schema failure.
 */

import type { SignalsQuery } from "@/lib/protocol/signals-query";

import { assertInBounds } from "../client/client-core";
import { apiGet } from "../client/server";
import {
  zListProtocolSignalsQuery,
  zListProtocolSignalsResponse,
  type PageResponseSignalResponse,
  type ProtocolResponse,
} from "../schema/zod.gen";

export const SIGNALS_PAGE_SIZE = 50;

/** @throws RangeError if the page size is outside the contract's bounds. */
export function fetchProtocolSignals(
  protocol: ProtocolResponse,
  query: SignalsQuery,
): Promise<PageResponseSignalResponse> {
  assertInBounds("limit", SIGNALS_PAGE_SIZE, zListProtocolSignalsQuery.shape.limit.unwrap());
  return apiGet(
    `/api/protocols/${protocol}/signals`,
    {
      limit: SIGNALS_PAGE_SIZE,
      severity: query.severity ?? undefined,
      cursor: query.page?.cursor,
      dir: query.page?.dir,
    },
    zListProtocolSignalsResponse,
  );
}
