/**
 * Pool protocol identifier → display label.
 *
 * Mapping is closed (a small switch) rather than reflective on the
 * string. Reasons:
 *
 *   - the indexer's set of protocols is finite and known here,
 *   - an unexpected value surfaces as `Unknown` instead of being
 *     blindly rendered with underscores,
 *   - i18n is easy to add later if we want localised protocol
 *     names — for now display labels are technical proper nouns
 *     ("Meteora DAMM v2") and stay in English regardless of UI
 *     locale.
 *
 * Add a case when a new protocol is wired through the indexer.
 */

export function formatProtocolLabel(protocol: string): string {
  switch (protocol) {
    case "meteora_damm_v2":
      return "Meteora DAMM v2";
    case "meteora_damm_v1":
      return "Meteora DAMM v1";
    case "meteora_dlmm":
      return "Meteora DLMM";
    case "meteora_stake2earn":
      return "Meteora Stake2Earn";
    default:
      return "Unknown";
  }
}