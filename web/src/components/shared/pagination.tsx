/**
 * First / Previous / Next / Last over cursor-paginated lists, as `<Link>`s: a Server
 * Component. `paramPrefix` lets several coexist on a page (`swapsCursor`, `liqDir`…).
 */

import { Link } from "@/i18n/navigation"; // adapt to your project's import
import { getTranslations } from "next-intl/server";

import {
  ChevronDoubleLeftIcon,
  ChevronLeftIcon,
  ChevronRightIcon,
  ChevronDoubleRightIcon,
} from "@/components/shared/icon"; // adapt to your icon module
import { buildHref } from "./pagination-href";

type PageView = {
  prevCursor: string | null;
  nextCursor: string | null;
  isFirst: boolean;
  isLast: boolean;
};

type PaginationProps = {
  /** A `PageResponse`'s navigation fields; `items` is not required. */
  page: PageView;

  /** Preserved in the hrefs: other paginations, filters, search. */
  searchParams: Record<string, string | string[] | undefined>;

  /** `""` for `cursor`/`dir`/`position`; `"swaps"` gives `swapsCursor`, `swapsDir`… */
  paramPrefix?: string;

  basePath: string;
};



const BTN_CLASS_BASE =
  "inline-flex items-center justify-center rounded-md h-9 w-9 " +
  "border border-sothoth-500/20 bg-cosmos-900/60 text-slate-300 " +
  "transition-colors hover:bg-cosmos-900/90 hover:text-slate-100 " +
  "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sothoth-400";

const BTN_CLASS_DISABLED =
  "inline-flex items-center justify-center rounded-md h-9 w-9 " +
  "border border-sothoth-500/10 bg-cosmos-900/30 text-slate-600 " +
  "cursor-not-allowed";

export async function Pagination({
  page,
  searchParams,
  paramPrefix = "",
  basePath,
}: PaginationProps) {
  const t = await getTranslations("Common.Pagination");

  const goFirst = !page.isFirst
    ? buildHref(basePath, searchParams, paramPrefix, { position: "first" })
    : null;

  const goPrev = !page.isFirst && page.prevCursor !== null
    ? buildHref(basePath, searchParams, paramPrefix, {
      cursor: page.prevCursor,
      dir: "prev",
    })
    : null;

  const goNext = !page.isLast && page.nextCursor !== null
    ? buildHref(basePath, searchParams, paramPrefix, {
      cursor: page.nextCursor,
      dir: "next",
    })
    : null;

  const goLast = !page.isLast
    ? buildHref(basePath, searchParams, paramPrefix, { position: "last" })
    : null;

  return (
    <nav
      aria-label={t("ariaLabel")}
      className="flex items-center justify-end gap-2 px-4 py-3"
    >
      <NavButton
        href={goFirst}
        ariaLabel={t("first")}
        icon={<ChevronDoubleLeftIcon className="h-4 w-4" />}
      />
      <NavButton
        href={goPrev}
        ariaLabel={t("previous")}
        icon={<ChevronLeftIcon className="h-4 w-4" />}
      />
      <NavButton
        href={goNext}
        ariaLabel={t("next")}
        icon={<ChevronRightIcon className="h-4 w-4" />}
      />
      <NavButton
        href={goLast}
        ariaLabel={t("last")}
        icon={<ChevronDoubleRightIcon className="h-4 w-4" />}
      />
    </nav>
  );
}

/** A `<Link>` when active, a `<span>` when not: not a `<button disabled>`, for link semantics. */
function NavButton({
  href,
  ariaLabel,
  icon,
}: {
  href: string | null;
  ariaLabel: string;
  icon: React.ReactNode;
}) {
  if (href === null) {
    return (
      <span
        role="link"
        aria-disabled="true"
        aria-label={ariaLabel}
        className={BTN_CLASS_DISABLED}
      >
        {icon}
      </span>
    );
  }
  return (
    <Link href={href} aria-label={ariaLabel} className={BTN_CLASS_BASE}>
      {icon}
    </Link>
  );
}
