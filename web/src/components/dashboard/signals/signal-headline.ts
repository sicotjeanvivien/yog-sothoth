import type { SignalResponse } from "@/lib/api/schema/zod.gen";
import { formatShortAddress } from "@/lib/format/format-short-address";

import { formatSignalValue, KNOWN_DETECTORS } from "./signal-display";

type HeadlineTranslator = (key: string, values: { pair: string; value: string }) => string;

/** A signal's sentence, by detector; `t` reads `Dashboard.Overview.signals`. */
export function signalHeadline(signal: SignalResponse, locale: string, t: HeadlineTranslator): string {
  // ⚠️ Isolated (FSI…PDI): a right-to-left symbol would otherwise reverse the whole line.
  const pair =
    signal.tokenA.symbol !== null && signal.tokenB.symbol !== null
      ? `⁨${signal.tokenA.symbol}⁩ / ⁨${signal.tokenB.symbol}⁩`
      : formatShortAddress(signal.poolAddress);
  const value = formatSignalValue(signal, locale);
  return KNOWN_DETECTORS.has(signal.detector)
    ? t(`headline.${signal.detector}`, { pair, value })
    : (signal.message ?? t("headline.fallback", { pair, value }));
}
