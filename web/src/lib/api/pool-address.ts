/**
 * Pool address validation — runtime-neutral.
 *
 * A pure syntactic guard with no runtime imports, so both server-side and
 * browser-side fetchers can share it without either pulling the other's
 * client (and its env) into their bundle.
 *
 * Base58 encodes 32 bytes into 43-44 characters from the Bitcoin alphabet
 * (excludes `0`, `O`, `I`, `l`). We check the shape only — verifying the bytes
 * round-trip into a valid Pubkey is yog-api's job; this rejects obviously
 * wrong input (empty string, URL injection) before going over the wire.
 */

const BASE58_PUBKEY = /^[1-9A-HJ-NP-Za-km-z]{32,44}$/;

export function isValidPoolAddress(address: string): boolean {
  return BASE58_PUBKEY.test(address);
}
