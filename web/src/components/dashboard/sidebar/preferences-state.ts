/**
 * The cookie remembering that the Preferences are open. ⚠️ No "use client": the server layout
 * reads it for the first paint, as it does the collapse cookie.
 */

export const PREFERENCES_OPEN_COOKIE = "yog_prefs_open";

export const PREFERENCES_COOKIE_MAX_AGE_S = 60 * 60 * 24 * 365;
