/** The committed contract as `/openapi.json` serves it. */

import spec from "@/lib/api/schema/openapi.json";

/** ⚠️ The file's own layout: two-space indent and a final newline, or the download differs. */
export function openApiBody(): string {
  return `${JSON.stringify(spec, null, 2)}\n`;
}
