/** Which protocols a page's figures add up: all of them on the Overview, one on its own page. */

import type { ProtocolResponse } from "@/lib/api/schema/zod.gen";

export type Scope = "all" | ProtocolResponse;

/** The rows a page in `scope` adds up, before any sum. */
export function inScope<T extends { protocol: ProtocolResponse }>(
  items: readonly T[],
  scope: Scope,
): T[] {
  return scope === "all" ? [...items] : items.filter((item) => item.protocol === scope);
}
