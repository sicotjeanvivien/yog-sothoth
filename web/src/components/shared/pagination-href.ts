import { PageDir, PagePosition } from "@/lib/api/type/pagination";

/** `("", "cursor")` → `"cursor"`, `("swaps", "cursor")` → `"swapsCursor"`. */
function namespacedKey(prefix: string, key: string): string {
  if (prefix.length === 0) return key;
  return prefix + key.charAt(0).toUpperCase() + key.slice(1);
}


/** Keeps every param outside this pagination's namespace and replaces its own three. */
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