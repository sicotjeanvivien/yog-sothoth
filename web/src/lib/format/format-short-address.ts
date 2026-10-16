/** Truncates an address to `head`, "..." and `tail` characters, 4 each by default. */

const DEFAULT_HEAD = 4;
const DEFAULT_TAIL = 4;

export function formatShortAddress(
  address: string,
  options?: { head?: number; tail?: number },
): string {
  const head = options?.head ?? DEFAULT_HEAD;
  const tail = options?.tail ?? DEFAULT_TAIL;

  if (address.length <= head + tail + 3) {
    return address;
  }

  return `${address.slice(0, head)}...${address.slice(-tail)}`;
}