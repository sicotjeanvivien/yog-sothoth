import type { ProtocolResponse } from "@/lib/api/schema/zod.gen";

/**
 * A protocol's display label. The switch is exhaustive over the contract, so a protocol the API
 * gains fails `typecheck` here until it has one. Labels are proper nouns, in English whatever the
 * locale.
 */
export function formatProtocolLabel(protocol: ProtocolResponse): string {
  switch (protocol) {
    case "meteora_damm_v2":
      return "Meteora DAMM v2";
    case "meteora_dlmm":
      return "Meteora DLMM";
  }
}
