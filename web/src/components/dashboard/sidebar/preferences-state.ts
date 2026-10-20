/**
 * The Preferences' state: the cookie remembering that they are open, and what a click does.
 * ⚠️ No "use client": the server layout reads the cookie for the first paint, as it does the
 * collapse cookie.
 */

export const PREFERENCES_OPEN_COOKIE = "yog_prefs_open";

export const PREFERENCES_COOKIE_MAX_AGE_S = 60 * 60 * 24 * 365;

/**
 * On the collapsed rail the panel has no room: a click unfolds the rail and opens it. ⚠️ The
 * drawer has no collapsed form, whatever the collapse cookie says.
 */
export function preferencesClick({
  collapsed,
  onRail,
  open,
}: {
  collapsed: boolean;
  onRail: boolean;
  open: boolean;
}): { open: boolean; expand: boolean } {
  return collapsed && onRail
    ? { open: true, expand: true }
    : { open: !open, expand: false };
}
