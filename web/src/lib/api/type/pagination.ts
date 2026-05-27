/**
 * Traversal direction relative to a cursor.
 *
 * `next` moves further into the list (older pools), `prev` moves
 * back toward newer pools. Defaults to `next` server-side when
 * unspecified.
 */
export type PageDir = "next" | "prev";

/**
 * Absolute jump to a list boundary, ignoring any cursor.
 */
export type PagePosition = "first" | "last";