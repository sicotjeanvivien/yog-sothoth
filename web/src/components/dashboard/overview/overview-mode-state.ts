/**
 * The Overview's advanced-mode cookie. ⚠️ No "use client": the server page reads it for the
 * first paint, and the switch writes it.
 */

export const OVERVIEW_ADVANCED_COOKIE = "yog_overview_advanced";

export const OVERVIEW_ADVANCED_COOKIE_MAX_AGE_S = 60 * 60 * 24 * 365;
