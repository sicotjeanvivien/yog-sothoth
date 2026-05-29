/**
 * Generic pagination control for cursor-based paginated lists.
 *
 * Four navigation buttons (First / Previous / Next / Last) bound
 * to URL search params. Renders as a Server Component: all
 * navigation is driven by `<Link>` href changes, which trigger an
 * RSC re-render of the parent page.
 *
 * `paramPrefix` lets multiple paginations coexist on the same
 * page (typically the pool detail page, which paginates swaps and
 * liquidity events independently). Each instance gets its own
 * namespace of search params: `cursor` / `dir` / `position` for an
 * unprefixed instance, `swapsCursor` / `swapsDir` / ... for a
 * prefixed one.
 *
 * Boundary flags `isFirst` / `isLast` come straight from the API
 * response and drive the disabled state of the buttons. A single-
 * page result has both flags true and all four buttons disabled.
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
  /**
   * The page metadata coming straight from a `PageResponse`. Only
   * the navigation fields are read; `items` is intentionally not
   * required so callers can pass a narrowed object.
   */
  page: PageView;

  /**
   * Current search params for this route. Used to preserve any
   * params unrelated to this pagination (other paginations,
   * filters, search query, etc.) when building the navigation
   * hrefs.
   */
  searchParams: Record<string, string | string[] | undefined>;

  /**
   * Param namespace for this pagination instance. `""` means the
   * params are unprefixed (`cursor`, `dir`, `position`). A non-empty
   * prefix produces camelCase param names — e.g. `"swaps"` gives
   * `swapsCursor`, `swapsDir`, `swapsPosition`.
   */
  paramPrefix?: string;

  /**
   * Base path for the navigation hrefs (typically the current
   * route). Search params are appended.
   */
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

/**
 * One navigation button. Renders as a `<Link>` when active, or as
 * a disabled-styled `<span>` when not. Avoids using a `<button
 * disabled>` because the link semantics matter for keyboard nav
 * and screen readers when active.
 */
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
