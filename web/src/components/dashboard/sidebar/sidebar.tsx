"use client";

/**
 * Dashboard sidebar — persistent left rail.
 *
 * Autonomous Client Component:
 *   - reads the current route itself via next-intl's `usePathname`
 *     (which returns the path *without* the locale segment, e.g.
 *     `/overview` — no manual locale stripping needed);
 *   - resolves its own labels via `useTranslations`;
 *   - reads the nav structure from `sidebar-nav.ts`.
 *
 * Because it is autonomous, the layout mounts it with no props:
 * `<Sidebar />`. Nothing upstream needs to know about routes or
 * active state.
 *
 * The footer (Solana Live status panel) and per-item icons are
 * intentionally out of scope for this iteration.
 */

import { useTranslations } from "next-intl";

import { Link, usePathname } from "@/i18n/navigation";

import { SIDEBAR_NAV } from "./sidebar-nav";

// ── Active-item logic ─────────────────────────────────────────────────

/**
 * Whether a nav entry is the active one.
 *
 * Exact match only — `/pools` is active solely on `/pools` itself,
 * not on `/pools/<address>`. `usePathname` from next-intl already
 * strips the locale, so both operands are locale-free paths and a
 * plain equality is enough.
 */
function isItemActive(pathname: string, href: string): boolean {
  return pathname === href;
}

// ── Component ─────────────────────────────────────────────────────────

export function Sidebar() {
  const pathname = usePathname();

  return (
    <aside className="flex h-screen w-60 shrink-0 flex-col border-r border-slate-800 bg-slate-950/60 px-6 py-8">
      <BrandBlock />
      <nav className="mt-10 flex-1">
        <SidebarNavList pathname={pathname} />
      </nav>
    </aside>
  );
}

// ── Sub-components (private) ──────────────────────────────────────────

/**
 * Brand block — placeholder "YS" mark, product name and tagline.
 * The mark is a temporary bordered square; an inline SVG eye will
 * replace it in a later iteration.
 */
function BrandBlock() {
  const t = useTranslations("Brand");

  return (
    <div className="flex flex-col items-center text-center">
      <div
        className="flex h-12 w-12 items-center justify-center rounded-md border border-slate-700 bg-slate-900 text-lg font-semibold tracking-widest text-slate-300"
        aria-hidden="true"
      >
        YS
      </div>
      <p className="mt-4 text-base font-semibold tracking-[0.18em] text-slate-200">
        {t("name")}
      </p>
      <p className="mt-1 text-[10px] uppercase tracking-[0.22em] text-slate-500">
        {t("tagline")}
      </p>
    </div>
  );
}

/**
 * The navigation list. Maps the static config to one link per entry.
 * Receives `pathname` from the parent so the active computation
 * happens against a single, consistent value.
 */
function SidebarNavList({ pathname }: { pathname: string }) {
  const t = useTranslations("Dashboard.Sidebar.nav");

  return (
    <ul className="space-y-1">
      {SIDEBAR_NAV.map((item) => (
        <li key={item.key}>
          <SidebarNavLink
            href={item.href}
            label={t(item.labelKey)}
            active={isItemActive(pathname, item.href)}
          />
        </li>
      ))}
    </ul>
  );
}

/**
 * A single navigation link. Purely presentational — it is told
 * whether it is active rather than working it out itself.
 *
 * Uses next-intl's `Link`, which prepends the active locale to
 * `href` automatically.
 */
function SidebarNavLink({
  href,
  label,
  active,
}: {
  href: string;
  label: string;
  active: boolean;
}) {
  const base =
    "flex items-center rounded-md px-3 py-2 text-sm transition-colors";
  const state = active
    ? "bg-slate-800 text-slate-100"
    : "text-slate-400 hover:bg-slate-800/60 hover:text-slate-100";

  return (
    <Link
      href={href}
      className={`${base} ${state}`}
      aria-current={active ? "page" : undefined}
    >
      {label}
    </Link>
  );
}