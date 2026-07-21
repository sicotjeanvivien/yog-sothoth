/**
 * Compact protocol cell — platform icon + short product label.
 *
 * Replaces the full "Meteora DAMM v2" text with a brand mark plus the short
 * label ("DAMM v2"): the icon carries the platform, so repeating "Meteora"
 * only wastes horizontal space in a dense table. Falls back to the full label
 * for any protocol we have no icon for.
 *
 * No hooks / no server-only imports, so the shared pool row can render it on
 * both `/pools` (server) and `/watchlist` (client). The full name is exposed
 * via `title` so it stays discoverable on hover and to assistive tech.
 */

import { MeteoraIcon } from "@/components/shared/icon";
import {
  formatProtocolLabel,
  formatProtocolShortLabel,
  protocolPlatform,
} from "@/lib/format/format-protocol";

export function ProtocolBadge({ protocol }: { protocol: string }) {
  const platform = protocolPlatform(protocol);

  if (platform === null) {
    return (
      <span className="text-slate-400">{formatProtocolLabel(protocol)}</span>
    );
  }

  return (
    <span
      className="inline-flex items-center gap-2 text-slate-300"
      title={formatProtocolLabel(protocol)}
    >
      <PlatformMark platform={platform} />
      <span>{formatProtocolShortLabel(protocol)}</span>
    </span>
  );
}

/** The platform's brand mark. Meteora today; add a case per platform as new
 *  ones (Raydium, Orca) are wired through `protocolPlatform`. */
function PlatformMark({ platform }: { platform: "meteora" }) {
  switch (platform) {
    case "meteora":
      return <MeteoraIcon size={16} className="shrink-0" />;
  }
}
