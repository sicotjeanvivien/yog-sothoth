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

/**
 * ⚠️ The URL in single quotes, where a `!` that `encodeURIComponent` leaves cannot expand; the
 * header in double quotes, where the shell must expand `$YOG_API_KEY` or `$TICKET`.
 */
export function curlOf(url: string, sse: boolean, method: string): string {
  if (sse) {
    return `curl -N -H 'Accept: text/event-stream' -H "Authorization: Bearer $TICKET" '${url}'`;
  }
  const auth = `-H "Authorization: Bearer $YOG_API_KEY"`;
  return method === "GET" ? `curl -s ${auth} '${url}'` : `curl -s -X ${method} ${auth} '${url}'`;
}

/**
 * The values that fetch the page `cursor` points at: its direction set where the route takes
 * `dir`, and `position` dropped, which the API refuses beside a cursor.
 */
export function pageValues(
  endpoint: Endpoint,
  values: ParamValues,
  cursor: string,
  dir: "next" | "prev",
): ParamValues {
  const next: ParamValues = { ...values, cursor };
  if (endpoint.params.some((p) => p.name === "position")) next["position"] = "";
  if (endpoint.params.some((p) => p.name === "dir")) next["dir"] = dir;
  return next;
}

export function missingOf(endpoint: Endpoint, values: ParamValues): string[] {
  return endpoint.params.filter((p) => p.required && !values[p.name]).map((p) => p.name);
}

/** The bytes of a body as the wire carried them, not its UTF-16 length. */
export function byteSize(text: string): number {
  return new TextEncoder().encode(text).length;
}
