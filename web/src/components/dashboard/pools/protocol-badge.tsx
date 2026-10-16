/**
 * The platform's mark and the short label, or the full label without a mark; the full name
 * stays in `title`. No hook, no server import: both `/pools` and `/watchlist` render it.
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

function PlatformMark({ platform }: { platform: "meteora" }) {
  switch (platform) {
    case "meteora":
      return <MeteoraIcon size={16} className="shrink-0" />;
  }
}
