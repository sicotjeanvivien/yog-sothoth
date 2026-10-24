/**
 * The tabs of a protocol's page, in the canvas's order, chosen by `?tab=`. A tab joins the row
 * when its content ships; its number is its place in the full list, so it never moves.
 */

export const PROTOCOL_TABS = ["liquidity", "volume", "fees", "pools", "tokens", "activity"] as const;

export type ProtocolTab = (typeof PROTOCOL_TABS)[number];

/** The tabs whose content has shipped. */
export const READY_TABS: ReadonlySet<ProtocolTab> = new Set<ProtocolTab>();

/** "01" to "06", the tab's index in the row and its keyboard shortcut. */
export function tabNumber(tab: ProtocolTab): string {
  return String(PROTOCOL_TABS.indexOf(tab) + 1).padStart(2, "0");
}

/** The ready tabs, in their row's order. */
export function readyTabs(ready: ReadonlySet<ProtocolTab> = READY_TABS): ProtocolTab[] {
  return PROTOCOL_TABS.filter((tab) => ready.has(tab));
}

/**
 * The tab asked for when it is ready, else the first ready one: a bad link still renders the
 * page. `null` while no tab has shipped.
 */
export function parseTab(
  raw: string | string[] | undefined,
  ready: ReadonlySet<ProtocolTab> = READY_TABS,
): ProtocolTab | null {
  const asked = PROTOCOL_TABS.find((tab) => tab === raw);
  if (asked !== undefined && ready.has(asked)) {
    return asked;
  }
  return readyTabs(ready)[0] ?? null;
}

/** The ready tab a digit key opens, by its number; `null` for any other key. */
export function tabForKey(
  key: string,
  ready: ReadonlySet<ProtocolTab> = READY_TABS,
): ProtocolTab | null {
  const tab = PROTOCOL_TABS[Number.parseInt(key, 10) - 1];
  return /^[1-9]$/.test(key) && tab !== undefined && ready.has(tab) ? tab : null;
}

/** The query a tab adds: none for the default tab, the first ready one, so a view has one URL. */
export function tabQuery(
  tab: ProtocolTab | null,
  ready: ReadonlySet<ProtocolTab> = READY_TABS,
): Record<string, string> {
  return tab === null || tab === readyTabs(ready)[0] ? {} : { tab };
}
