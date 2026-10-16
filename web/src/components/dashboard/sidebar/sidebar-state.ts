/**
 * The sidebar collapse cookie. ⚠️ No "use client": the server layout reads it for the first
 * paint, which localStorage could not serve, and the client shell writes it.
 */

export const SIDEBAR_COLLAPSED_COOKIE = "yog_sidebar_collapsed";

export const SIDEBAR_COOKIE_MAX_AGE_S = 60 * 60 * 24 * 365;
