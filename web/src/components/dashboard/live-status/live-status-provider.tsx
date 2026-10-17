"use client";

/**
 * One `/api/ingestion` poll for the whole dashboard page, shared by the menu's dot and the
 * header's indicator. ⚠️ Outside the provider, the hook reads "connecting", never throws.
 */

import { createContext, useContext, type ReactNode } from "react";

import { fetchIngestionBrowser } from "@/lib/api/browser/ingestion";
import type { IngestionResponse } from "@/lib/api/schema/zod.gen";

import { usePolled, type Half } from "./use-polled";

const IngestionContext = createContext<Half<IngestionResponse>>({
  phase: "loading",
});

export function LiveStatusProvider({ children }: { children: ReactNode }) {
  const ingestion = usePolled(fetchIngestionBrowser);
  return (
    <IngestionContext.Provider value={ingestion}>
      {children}
    </IngestionContext.Provider>
  );
}

export function useIngestion(): Half<IngestionResponse> {
  return useContext(IngestionContext);
}

/** A fixed ingestion state, with no fetch: what the tests mount. */
export const IngestionContextProvider = IngestionContext.Provider;
