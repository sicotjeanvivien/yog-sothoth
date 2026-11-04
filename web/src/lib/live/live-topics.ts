/**
 * The topics of `/api/live` a page listens to, and the one URL that carries them all: a page opens
 * a single connection, whatever its topics.
 */

import type { ProtocolResponse } from "@/lib/api/schema/zod.gen";

/** The on-chain collections streamed live, every one but the swaps. */
export const ACTIVITY_TOPICS = [
  "liquidity-changes",
  "pool-creations",
  "position-changes",
  "position-fee-claims",
  "protocol-fee-claims",
  "reward-claims",
  "reward-changes",
  "pool-changes",
] as const;

export type ActivityTopic = (typeof ACTIVITY_TOPICS)[number];

export type LiveTopic = "signals" | ActivityTopic;

/** The signals alone in the simple mode; the activity table's collections join them in the advanced. */
export function pageTopics(advanced: boolean): readonly LiveTopic[] {
  return advanced ? ["signals", ...ACTIVITY_TOPICS] : ["signals"];
}

/** Every protocol's stream, `/api/live`, or one protocol's, under `/api/live/protocols/{protocol}`. */
export function livePath(topics: readonly LiveTopic[], protocol?: ProtocolResponse): string {
  const scope = protocol === undefined ? "/api/live" : `/api/live/protocols/${protocol}`;
  return `${scope}?topics=${topics.join(",")}`;
}
