/** Which protocols a page's figures add up: all of them on the Overview, one on its own page. */

import type { ProtocolResponse } from "../schema/zod.gen";

export type Scope = "all" | ProtocolResponse;

/** The route of `resource` in `scope`: every protocol's at the root, else the protocol's own. */
export function scopedPath(scope: Scope, resource: string): string {
  return scope === "all" ? `/api/${resource}` : `/api/protocols/${scope}/${resource}`;
}
