/**
 * Sidebar navigation keys.
 *
 * Source of truth for the set of identifiers a navigation entry can
 * carry. Defined as a literal union so TypeScript rejects any typo at
 * compile time (`"oerview"` will not type-check).
 *
 * This is a leaf module — it imports nothing. Both `sidebar-nav.ts`
 * (to type its config entries) and `sidebar.tsx` (to type what it
 * manipulates) depend on it, and it depends on nothing in return.
 *
 * Add a key here when a new dashboard section ships, in lockstep with
 * its entry in `sidebar-nav.ts`.
 */

export type SidebarNavKey = "overview" | "pools" | "signals" | "watchlist";