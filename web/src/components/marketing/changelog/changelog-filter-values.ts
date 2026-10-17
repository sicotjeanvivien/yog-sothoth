/** The changelog's filters, shared by the client bar and the server list that hides by CSS. */

import type { ReleaseSectionKind } from "./releases";

export type ChangelogFilterValue = "all" | ReleaseSectionKind;

export const FILTERS: readonly ChangelogFilterValue[] = ["all", "features", "fixes"];

/** What hides an element under each section filter; literal, for Tailwind to find. */
export const HIDDEN_ON: Record<ReleaseSectionKind, string> = {
  features: "group-data-[filter=features]/changelog:hidden",
  fixes: "group-data-[filter=fixes]/changelog:hidden",
};
