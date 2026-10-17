/** What a release shows: its visible lines by section, its technical ones folded apart. */

import type { Release } from "./releases";

export type ReleaseView = {
  features: readonly string[];
  fixes: readonly string[];
  technical: readonly string[];
  /** No visible line: the page says so in place of the summary. */
  internalOnly: boolean;
};

export function releaseView(release: Release): ReleaseView {
  const visible = (kind: "features" | "fixes") =>
    release.sections
      .filter((section) => section.kind === kind)
      .flatMap((section) => section.items)
      .filter((item) => !item.technical)
      .map((item) => item.text);
  const features = visible("features");
  const fixes = visible("fixes");
  const technical = release.sections
    .flatMap((section) => section.items)
    .filter((item) => item.technical)
    .map((item) => item.text);

  return {
    features,
    fixes,
    technical,
    internalOnly: features.length === 0 && fixes.length === 0,
  };
}
