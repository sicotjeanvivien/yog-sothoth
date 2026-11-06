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

/** The topics a live URL asks for: the names its events carry. */
export function topicsOf(url: string): string[] {
  const topics = new URL(url).searchParams.get("topics") ?? "";
  return topics.split(",").filter((topic) => topic !== "");
}

/** `json` on one line; as typed while it does not parse yet. */
function compact(json: string): string {
  try {
    return JSON.stringify(JSON.parse(json));
  } catch {
    return json;
  }
}

/** How the page sends a request: `keyed` when the visitor gave a key. */
export type CurlRequest = { sse: boolean; body: string | null; keyed: boolean };

/** ⚠️ Single quotes around the URL and the body, where a `!` would expand; double ones around
 *  `Authorization`, so that the shell reads the visitor's variable: the key itself is never written. */
export function curlOf(url: string, request: CurlRequest): string {
  if (request.sse) {
    return `curl -N -H 'Accept: text/event-stream' -H "Authorization: Bearer $YOG_LIVE_TOKEN" '${url}'`;
  }
  const key = request.keyed || request.body !== null ? ` -H "Authorization: Bearer $YOG_API_KEY"` : "";
  if (request.body !== null) {
    const body = compact(request.body).replaceAll("'", "'\\''");
    return `curl -s -X POST${key} -H 'Content-Type: application/json' -d '${body}' '${url}'`;
  }
  return `curl -s${key} '${url}'`;
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
