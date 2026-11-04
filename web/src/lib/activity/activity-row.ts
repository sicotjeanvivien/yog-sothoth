/**
 * One line of the live activity table, from an item of its collection: its kind, its pool by
 * pair, and a short detail — amounts in units of the token, then who. Pure presentation: no
 * dollar, and no amount whose decimals are unknown.
 */

import {
  zLiquidityChangeEventResponse,
  zPoolChangeEventResponse,
  zPoolCreationEventResponse,
  zPositionChangeEventResponse,
  zPositionFeeClaimEventResponse,
  zProtocolFeeClaimEventResponse,
  zRewardChangeEventResponse,
  zRewardClaimEventResponse,
  type EmbeddedTokenResponse,
  type LiquidityChangeEventResponse,
  type PoolChangeEventResponse,
  type PoolCreationEventResponse,
  type PositionChangeEventResponse,
  type PositionFeeClaimEventResponse,
  type ProtocolFeeClaimEventResponse,
  type ProtocolResponse,
  type RewardChangeEventResponse,
  type RewardClaimEventResponse,
} from "@/lib/api/schema/zod.gen";
import { formatTokenAmount } from "@/lib/format/format-token-amount";
import { shortenPubkey } from "@/lib/format/pubkey";
import type { ActivityTopic } from "@/lib/live/live-topics";

/** An item as it arrives, tagged by the topic that named it. */
export type ActivityItem =
  | { topic: "liquidity-changes"; event: LiquidityChangeEventResponse }
  | { topic: "pool-creations"; event: PoolCreationEventResponse }
  | { topic: "position-changes"; event: PositionChangeEventResponse }
  | { topic: "position-fee-claims"; event: PositionFeeClaimEventResponse }
  | { topic: "protocol-fee-claims"; event: ProtocolFeeClaimEventResponse }
  | { topic: "reward-claims"; event: RewardClaimEventResponse }
  | { topic: "reward-changes"; event: RewardChangeEventResponse }
  | { topic: "pool-changes"; event: PoolChangeEventResponse };

export const ACTIVITY_KINDS = [
  "liquidity_add",
  "liquidity_remove",
  "pool_creation",
  "position_open",
  "position_close",
  "position_vesting_lock",
  "position_permanent_lock",
  "position_split",
  "position_change",
  "position_fee_claim",
  "protocol_fee_claim",
  "reward_claim",
  "reward_initialize",
  "reward_fund",
  "reward_duration_change",
  "reward_funder_change",
  "reward_ineligible_withdrawal",
  "reward_dead_liquidity_withdrawal",
  "reward_change",
  "pool_status",
  "pool_fees",
  "pool_change",
] as const;

/** Its label's key; `position_change`, `reward_change`, `pool_change` name a protocol's change without its own block. */
export type ActivityKind = (typeof ACTIVITY_KINDS)[number];

export type PartyRole = "position" | "owner" | "creator" | "funder" | "reward" | "to";

export type DetailPart =
  | { type: "amount"; sign: "+" | "−" | ""; value: string; unit: string }
  | { type: "party"; role: PartyRole; address: string };

export type ActivityRow = {
  key: string;
  at: string;
  protocol: ProtocolResponse;
  kind: ActivityKind;
  pool: string;
  detail: readonly DetailPart[];
};

type Pair = { tokenA: EmbeddedTokenResponse; tokenB: EmbeddedTokenResponse };

function amount(raw: string, token: EmbeddedTokenResponse, sign: "+" | "−" | "" = ""): DetailPart[] {
  const value = formatTokenAmount(raw, token.decimals);
  if (value === null) {
    return [];
  }
  const unit = token.symbol ?? (token.mint === null ? "" : shortenPubkey(token.mint));
  return [{ type: "amount", sign, value, unit }];
}

/** An amount of `mint`, readable only when it is one side of the pair, whose decimals are known. */
function amountOf(raw: string, mint: string, pair: Pair): DetailPart[] {
  const side = [pair.tokenA, pair.tokenB].find((token) => token.mint === mint);
  return side === undefined ? [] : amount(raw, side);
}

function party(role: PartyRole, address: string): DetailPart {
  return { type: "party", role, address: shortenPubkey(address) };
}

type KindAndDetail = { kind: ActivityKind; detail: DetailPart[] };

function positionChange(e: PositionChangeEventResponse): KindAndDetail {
  const change = e.meteoraDammV2;
  switch (change?.kind) {
    case "open":
    case "close":
      return {
        kind: change.kind === "open" ? "position_open" : "position_close",
        detail: [party("position", change.position), party("owner", change.owner)],
      };
    case "vesting_lock":
      return { kind: "position_vesting_lock", detail: [party("position", change.position)] };
    case "permanent_lock":
      return { kind: "position_permanent_lock", detail: [party("position", change.position)] };
    case "split":
      return {
        kind: "position_split",
        detail: [party("position", change.firstPosition), party("to", change.secondPosition)],
      };
    case undefined:
      return { kind: "position_change", detail: [] };
  }
}

function rewardChange(e: RewardChangeEventResponse): KindAndDetail {
  const change = e.meteoraDammV2;
  switch (change?.kind) {
    case "initialize":
      return {
        kind: "reward_initialize",
        detail: [party("reward", change.rewardMint), party("funder", change.funder)],
      };
    case "fund":
      return {
        kind: "reward_fund",
        detail: [...amountOf(change.amount, change.rewardMint, e), party("funder", change.funder)],
      };
    case "duration_change":
      return { kind: "reward_duration_change", detail: [] };
    case "funder_change":
      return { kind: "reward_funder_change", detail: [party("funder", change.newFunder)] };
    case "ineligible_withdrawal":
      return { kind: "reward_ineligible_withdrawal", detail: amountOf(change.amount, change.rewardMint, e) };
    case "dead_liquidity_withdrawal":
      return {
        kind: "reward_dead_liquidity_withdrawal",
        detail: amountOf(change.amount, change.rewardMint, e),
      };
    case undefined:
      return { kind: "reward_change", detail: [] };
  }
}

function kindAndDetail(item: ActivityItem): KindAndDetail {
  switch (item.topic) {
    case "liquidity-changes": {
      const e = item.event;
      const sign = e.kind === "add" ? "+" : "−";
      return {
        kind: e.kind === "add" ? "liquidity_add" : "liquidity_remove",
        detail: [
          ...amount(e.amountA, e.tokenA, sign),
          ...amount(e.amountB, e.tokenB, sign),
          party("position", e.position),
        ],
      };
    }
    case "pool-creations":
      return { kind: "pool_creation", detail: [party("creator", item.event.creator)] };
    case "position-changes":
      return positionChange(item.event);
    case "position-fee-claims": {
      const e = item.event;
      return {
        kind: "position_fee_claim",
        detail: [...amount(e.feeA, e.tokenA), ...amount(e.feeB, e.tokenB), party("owner", e.owner)],
      };
    }
    case "protocol-fee-claims": {
      const e = item.event;
      return {
        kind: "protocol_fee_claim",
        detail: [...amount(e.amountA, e.tokenA), ...amount(e.amountB, e.tokenB)],
      };
    }
    case "reward-claims": {
      const e = item.event;
      return {
        kind: "reward_claim",
        detail: [...amountOf(e.amount, e.rewardMint, e), party("owner", e.owner)],
      };
    }
    case "reward-changes":
      return rewardChange(item.event);
    case "pool-changes": {
      const change = item.event.meteoraDammV2;
      if (change === undefined) {
        return { kind: "pool_change", detail: [] };
      }
      return { kind: change.kind === "status" ? "pool_status" : "pool_fees", detail: [] };
    }
  }
}

/** The key a line keeps across renders: an event is one `(signature, index)` of its collection. */
export function activityKey(topic: ActivityTopic, event: { signature: string; eventIndex: number }) {
  return `${topic}:${event.signature}:${event.eventIndex}`;
}

export function activityRow(item: ActivityItem): ActivityRow {
  const { kind, detail } = kindAndDetail(item);
  const e = item.event;
  return {
    key: activityKey(item.topic, e),
    at: e.at,
    protocol: e.protocol,
    kind,
    pool: poolLabel(e),
    detail,
  };
}

/** The pool by its pair, its short address while a symbol is unknown.
 * ⚠️ Each symbol isolated (FSI…PDI): a right-to-left one would reverse the line. */
function poolLabel(e: Pair & { poolAddress: string }): string {
  return e.tokenA.symbol !== null && e.tokenB.symbol !== null
    ? `\u2068${e.tokenA.symbol}\u2069 / \u2068${e.tokenB.symbol}\u2069`
    : shortenPubkey(e.poolAddress);
}

/** A message of `topic` as its collection's item, or `null` when it is not one. */
export function toActivityItem(topic: ActivityTopic, raw: unknown): ActivityItem | null {
  switch (topic) {
    case "liquidity-changes": {
      const parsed = zLiquidityChangeEventResponse.safeParse(raw);
      return parsed.success ? { topic, event: parsed.data } : null;
    }
    case "pool-creations": {
      const parsed = zPoolCreationEventResponse.safeParse(raw);
      return parsed.success ? { topic, event: parsed.data } : null;
    }
    case "position-changes": {
      const parsed = zPositionChangeEventResponse.safeParse(raw);
      return parsed.success ? { topic, event: parsed.data } : null;
    }
    case "position-fee-claims": {
      const parsed = zPositionFeeClaimEventResponse.safeParse(raw);
      return parsed.success ? { topic, event: parsed.data } : null;
    }
    case "protocol-fee-claims": {
      const parsed = zProtocolFeeClaimEventResponse.safeParse(raw);
      return parsed.success ? { topic, event: parsed.data } : null;
    }
    case "reward-claims": {
      const parsed = zRewardClaimEventResponse.safeParse(raw);
      return parsed.success ? { topic, event: parsed.data } : null;
    }
    case "reward-changes": {
      const parsed = zRewardChangeEventResponse.safeParse(raw);
      return parsed.success ? { topic, event: parsed.data } : null;
    }
    case "pool-changes": {
      const parsed = zPoolChangeEventResponse.safeParse(raw);
      return parsed.success ? { topic, event: parsed.data } : null;
    }
  }
}
