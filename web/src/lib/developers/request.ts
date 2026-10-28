/** The request the developers page sends: its URL, its `curl`, and what it still lacks. */

import type { Endpoint } from "./contract";

export type ParamValues = Record<string, string>;

/** An empty path parameter stays `{name}`; an empty query parameter is left out. */
export function urlOf(base: string, endpoint: Endpoint, values: ParamValues): string {
  let path = endpoint.path;
  const query: string[] = [];
  for (const p of endpoint.params) {
    const value = values[p.name] ?? "";
    if (p.in === "path") {
      if (value) path = path.replace(`{${p.name}}`, encodeURIComponent(value));
    } else if (value) {
      query.push(`${p.name}=${encodeURIComponent(value)}`);
    }
  }
  return `${base}${path}${query.length ? `?${query.join("&")}` : ""}`;
}

export function curlOf(url: string, sse: boolean): string {
  return sse ? `curl -N -H "Accept: text/event-stream" "${url}"` : `curl -s "${url}"`;
}

export function missingOf(endpoint: Endpoint, values: ParamValues): string[] {
  return endpoint.params.filter((p) => p.required && !values[p.name]).map((p) => p.name);
}

/** The bytes of a body as the wire carried them, not its UTF-16 length. */
export function byteSize(text: string): number {
  return new TextEncoder().encode(text).length;
}
