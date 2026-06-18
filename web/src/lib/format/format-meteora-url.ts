/**
 * Pool protocol identifier + address → Meteora app URL.
 *
 * The Meteora app routes each product under its own path segment
 * (`dammv2`, `dlmm`), so the segment is protocol-dependent — it is
 * *not* a constant `/pools/`.
 *
 * Mapping is a closed switch, mirroring `formatProtocolLabel`: the
 * indexer's protocol set is finite and known here, and an unexpected
 * (or not-yet-mapped) value returns `null` so the caller hides the
 * link rather than producing a broken URL.
 *
 * Add a case when a new protocol's Meteora route is known.
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
