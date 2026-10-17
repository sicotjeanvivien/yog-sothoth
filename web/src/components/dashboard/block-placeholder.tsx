/**
 * The waiting slot of one block while its data streams in: a flat surface of roughly the
 * block's size, so the page does not jump when the block arrives.
 */

import { getTranslations } from "next-intl/server";

export async function BlockPlaceholder({ className }: { className: string }) {
  const t = await getTranslations("Dashboard.shell");

  return (
    <div
      aria-busy="true"
      className={`border border-dash-rule bg-dash-surface ${className}`}
    >
      <span className="sr-only">{t("loading")}</span>
    </div>
  );
}
