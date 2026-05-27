import { PageDir, PagePosition } from "@/lib/api/type/pagination";

/**
 * Build a search-param key with optional camelCase prefixing.
 *
 *   namespacedKey("",      "cursor") === "cursor"
 *   namespacedKey("swaps", "cursor") === "swapsCursor"
 *   namespacedKey("liq",   "dir")    === "liqDir"
 */
function namespacedKey(prefix: string, key: string): string {
  if (prefix.length === 0) return key;
  return prefix + key.charAt(0).toUpperCase() + key.slice(1);
}


/**
 * Build the href for a navigation button.
 *
 * Preserves every search param that doesn't belong to this
 * pagination's namespace (other paginations, filters, search).
 * Clears the three params owned by this namespace, then sets only
 * the ones relevant to the target navigation.
 */
export function buildHref(
  basePath: string,
  searchParams: Record<string, string | string[] | undefined>,
  prefix: string,
  target: { cursor?: string; dir?: PageDir; position?: PagePosition },
): string {
  const cursorKey = namespacedKey(prefix, "cursor");
  const dirKey = namespacedKey(prefix, "dir");
  const positionKey = namespacedKey(prefix, "position");

  const next = new URLSearchParams();

  // Carry over everything that isn't owned by this pagination.
  for (const [key, value] of Object.entries(searchParams)) {
    if (key === cursorKey || key === dirKey || key === positionKey) continue;
    if (value === undefined) continue;
    if (Array.isArray(value)) {
      for (const v of value) next.append(key, v);
    } else {
      next.set(key, value);
    }
  }

  if (target.cursor !== undefined) next.set(cursorKey, target.cursor);
  if (target.dir !== undefined) next.set(dirKey, target.dir);
  if (target.position !== undefined) next.set(positionKey, target.position);

  const qs = next.toString();
  return qs.length > 0 ? `${basePath}?${qs}` : basePath;
}