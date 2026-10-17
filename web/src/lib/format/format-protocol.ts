/**
 * A protocol id's display label, from a closed switch: an unknown value shows `Unknown`, not
 * its underscores. Labels are proper nouns, in English whatever the locale. Add a case with
 * each protocol the indexer gains.
 */

export function formatProtocolLabel(protocol: string): string {
  switch (protocol) {
    case "meteora_damm_v2":
      return "Meteora DAMM v2";
    case "meteora_dlmm":
      return "Meteora DLMM";
    case "meteora_stake2earn":
      return "Meteora Stake2Earn";
    default:
      return "Unknown";
  }
}
