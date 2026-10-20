"use client";

/**
 * The advanced mode: an attribute over the page that the `adv:` variant reads. ⚠️ The server
 * renders both modes' content; switching shows or hides it, and never fetches again.
 */

import {
  createContext,
  useCallback,
  useContext,
  useState,
  useSyncExternalStore,
  type ReactNode,
} from "react";

import { writeCookie } from "@/components/shared/write-cookie";

import {
  OVERVIEW_ADVANCED_COOKIE,
  OVERVIEW_ADVANCED_COOKIE_MAX_AGE_S,
} from "./overview-mode-state";

const ModeContext = createContext<{ advanced: boolean; toggle: () => void } | null>(null);

const subscribeNever = () => () => {};

function cookieSaysAdvanced(): boolean {
  return document.cookie.split("; ").includes(`${OVERVIEW_ADVANCED_COOKIE}=1`);
}

export function OverviewMode({
  initialAdvanced,
  children,
}: {
  /** Read from the cookie by the page, so the first paint is in the right mode. */
  initialAdvanced: boolean;
  children: ReactNode;
}) {
  // ⚠️ Read from the cookie at each mount: Back restores the page with the props of its first
  // render. The local choice wins after a toggle, so a refused cookie still switches.
  const fromCookie = useSyncExternalStore(subscribeNever, cookieSaysAdvanced, () => initialAdvanced);
  const [chosen, setChosen] = useState<boolean | null>(null);
  const advanced = chosen ?? fromCookie;
  const toggle = useCallback(() => {
    writeCookie(OVERVIEW_ADVANCED_COOKIE, !advanced, OVERVIEW_ADVANCED_COOKIE_MAX_AGE_S);
    setChosen(!advanced);
  }, [advanced]);

  return (
    <ModeContext.Provider value={{ advanced, toggle }}>
      <div data-overview-advanced={advanced ? "" : undefined}>{children}</div>
    </ModeContext.Provider>
  );
}

export function useOverviewMode() {
  const mode = useContext(ModeContext);
  if (mode === null) {
    throw new Error("useOverviewMode outside OverviewMode");
  }
  return mode;
}
