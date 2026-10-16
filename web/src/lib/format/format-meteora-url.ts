/**
 * A pool's Meteora app URL; the path segment depends on the protocol (`dammv2`, `dlmm`). An
 * unmapped protocol is `null`, and the caller hides the link. Add a case per protocol.
 */

export function formatMeteoraUrl(
  protocol: string,
  poolAddress: string,
): string | null {
  switch (protocol) {
    case "meteora_damm_v2":
      return `https://app.meteora.ag/dammv2/${poolAddress}`;
    case "meteora_dlmm":
      return `https://app.meteora.ag/dlmm/${poolAddress}`;
    default:
      return null;
  }
}
