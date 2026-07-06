/**
 * Sidebar collapse state — shared vocabulary.
 *
 * Plain module (no "use client") so both sides of the boundary can
 * import it: the dashboard layout (Server Component) reads the cookie
 * to render the correct initial state — no expanded→collapsed flash
 * on load, which localStorage could not avoid — and the shell (client)
 * writes it on toggle.
 */

export const SIDEBAR_COLLAPSED_COOKIE = "yog_sidebar_collapsed";

/** One year — the preference should outlive any session. */
export const SIDEBAR_COOKIE_MAX_AGE_S = 60 * 60 * 24 * 365;
