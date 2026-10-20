/** An on/off preference cookie, `"1"` or `"0"`, for the whole site. */
export function writeCookie(name: string, on: boolean, maxAgeS: number) {
  document.cookie = `${name}=${on ? "1" : "0"}; path=/; max-age=${maxAgeS}; samesite=lax`;
}
