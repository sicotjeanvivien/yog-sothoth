/**
 * The shape of a pool address, with no import, so server and browser fetchers share it.
 * The bytes are yog-api's to check.
 */

const BASE58_PUBKEY = /^[1-9A-HJ-NP-Za-km-z]{32,44}$/;

export function isValidPoolAddress(address: string): boolean {
  return BASE58_PUBKEY.test(address);
}
