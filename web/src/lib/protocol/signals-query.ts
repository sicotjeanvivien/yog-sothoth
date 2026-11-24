/** Where the Signals tab stands in the protocol's list, in the URL under the API's own names. */

import type { SeverityResponse } from "@/lib/api/schema/zod.gen";

import { tabQuery, type ProtocolTab } from "./tabs";

export type SignalsQuery = {
  severity: SeverityResponse | null;
  /** `null` on the first page. */
  page: { cursor: string; dir: "next" | "prev" } | null;
};

const SEVERITIES: readonly SeverityResponse[] = ["info", "warning", "critical"];

type RawQuery = {
  severity?: string | string[];
  cursor?: string | string[];
  dir?: string | string[];
};

/** A missing, repeated or unknown value reads as none: unfiltered, on the first page. */
export function readSignalsQuery(raw: RawQuery): SignalsQuery {
  const severity = SEVERITIES.find((s) => s === raw.severity) ?? null;
  const cursor = typeof raw.cursor === "string" && raw.cursor !== "" ? raw.cursor : null;
  return {
    severity,
    page: cursor === null ? null : { cursor, dir: raw.dir === "prev" ? "prev" : "next" },
  };
}

/** The URL's parameters for a query; `dir=next`, the default, is left out so a view has one URL. */
export function signalsQueryParams(query: SignalsQuery): Record<string, string> {
  return {
    ...(query.severity === null ? {} : { severity: query.severity }),
    ...(query.page === null ? {} : { cursor: query.page.cursor }),
    ...(query.page?.dir === "prev" ? { dir: "prev" } : {}),
  };
}

/** What a period change keeps: the tab, and on the Signals tab its filter and page. */
export function periodKeep(tab: ProtocolTab | null, signals: SignalsQuery): Record<string, string> {
  return tab === "signals" ? { ...tabQuery(tab), ...signalsQueryParams(signals) } : tabQuery(tab);
}
