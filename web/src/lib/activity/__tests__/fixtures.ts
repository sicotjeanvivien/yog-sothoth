import type { EmbeddedTokenResponse } from "@/lib/api/schema/zod.gen";

export const MET: EmbeddedTokenResponse = {
  mint: "METvsvVRapdj9cFLzq4Tr43xK4tAjQfwX76z3n6mWQL",
  symbol: "MET",
  name: "Meteora",
  decimals: 6,
  logoUri: null,
};

export const SOL: EmbeddedTokenResponse = {
  mint: "So11111111111111111111111111111111111111112",
  symbol: "SOL",
  name: "Wrapped SOL",
  decimals: 9,
  logoUri: null,
};

/** A token yog-context has not fetched yet. */
export const UNKNOWN: EmbeddedTokenResponse = {
  mint: "F2PqFARmZgJRpz4KvB1iTexDfbwVy2uXs1xCYDgUBQup",
  symbol: null,
  name: null,
  decimals: null,
  logoUri: null,
};

export const POSITION = "9fQ2aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaL8xa";
export const OWNER = "5hTrbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbWq1c";
export const POOL = "ELR1TPLpcccccccccccccccccccccccccccccccc9xQz";

export const BASE = {
  protocol: "meteora_damm_v2",
  poolAddress: POOL,
  signature: "3L3RY5sT8K4kyEnqhizwaqxLEbcYvpGrGPNEYRwtbCSU",
  eventIndex: 0,
  slot: 9,
  at: "2026-11-04T21:14:05Z",
  tokenA: MET,
  tokenB: SOL,
} as const;
