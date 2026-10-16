/**
 * The shape of a pool address, with no import, so server and browser fetchers share it:
 * base58 that decodes to exactly 32 bytes, as yog-api's `Pubkey::from_str` requires.
 */

const BASE58_ALPHABET =
  "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
const BASE58_PUBKEY = /^[1-9A-HJ-NP-Za-km-z]{32,44}$/;
const PUBKEY_BYTES = 32;

export function isValidPoolAddress(address: string): boolean {
  return (
    BASE58_PUBKEY.test(address) && base58ByteLength(address) === PUBKEY_BYTES
  );
}

function base58ByteLength(encoded: string): number {
  let value = 0n;
  for (const char of encoded) {
    value = value * 58n + BigInt(BASE58_ALPHABET.indexOf(char));
  }
  // ⚠️ Each leading "1" is a zero byte that the numeric value does not carry.
  const leadingZeroBytes = encoded.length - encoded.replace(/^1+/, "").length;
  const valueBytes = value === 0n ? 0 : Math.ceil(value.toString(16).length / 2);
  return leadingZeroBytes + valueBytes;
}
