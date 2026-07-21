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

/**
 * Short protocol label — the product name without the platform prefix
 * ("DAMM v2" rather than "Meteora DAMM v2"). Meant to sit next to the
 * platform icon in dense cells, where the icon already carries the brand,
 * so repeating "Meteora" wastes horizontal space.
 */
export function formatProtocolShortLabel(protocol: string): string {
  switch (protocol) {
    case "meteora_damm_v2":
      return "DAMM v2";
    case "meteora_damm_v1":
      return "DAMM v1";
    case "meteora_dlmm":
      return "DLMM";
    case "meteora_stake2earn":
      return "Stake2Earn";
    default:
      return "Unknown";
  }
}

/**
 * The platform a protocol belongs to — drives which brand icon a compact
 * protocol cell shows. `null` when we have no icon for it (the cell then
 * falls back to the full label).
 */
export function protocolPlatform(protocol: string): "meteora" | null {
  return protocol.startsWith("meteora_") ? "meteora" : null;
}