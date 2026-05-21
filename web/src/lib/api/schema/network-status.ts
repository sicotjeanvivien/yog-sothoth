/**
 * Schema for the `GET /api/network/status` response.
 *
 * Mirrors the `NetworkStatusResponse` DTO emitted by yog-api. The
 * payload combines two concerns: the chain link (slot + RPC latency,
 * from the `network_status` singleton) and ingestion freshness
 * (derived server-side from the most recent indexed event).
 *
 * Notes on a couple of fields:
 *   - `slot` is a STRING on the wire — slots are u64 and can exceed
 *     the JS safe-integer range, so yog-api serialises them as text.
 *     Kept as a string here; the UI displays it verbatim and never
 *     does arithmetic on it.
 *   - `freshness` is a closed enum — `z.enum` rejects any value
 *     outside the three known tags, so an unexpected verdict surfaces
 *     as a validation failure rather than leaking into the UI.
 */

import * as z from "zod";

/** The three ingestion-freshness verdicts yog-api can return. */
export const FreshnessSchema = z.enum(["live", "delayed", "stale"]);

/** Freshness verdict — `"live" | "delayed" | "stale"`. */
export type Freshness = z.infer<typeof FreshnessSchema>;

/** Schema for the full network status payload. */
export const NetworkStatusSchema = z.object({
  // u64 slot, serialised as a string by yog-api (see file header).
  slot: z.string(),
  // getSlot round-trip latency measured by the indexer's reporter.
  rpcLatencyMs: z.number(),
  // When the indexer recorded the slot above.
  observedAt: z.string(),
  // Ingestion freshness verdict.
  freshness: FreshnessSchema,
  // Timestamp of the most recent indexed event; null on an empty DB.
  lastEventAt: z.string().nullable(),
});

/** Validated network status payload. */
export type NetworkStatusResponse = z.infer<typeof NetworkStatusSchema>;