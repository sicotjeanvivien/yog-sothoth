/**
 * Dashboard layout — wraps every authenticated dashboard route with a
 * fixed left sidebar containing the brand mark and the primary
 * navigation.
 *
 * The route group `(dashboard)` keeps this layout scoped: the locale
 * home page (`[locale]/page.tsx`) does NOT inherit it, so the landing
 * stays minimalist while the dashboard takes the full chrome.
 *
 * Server Component — fetches translations on the server, no client
 * JS shipped for the chrome itself. Active-link highlighting will
 * arrive when we have more than one route to navigate to.
 */

import { getTranslations, setRequestLocale } from "next-intl/server";

type DashboardLayoutProps = {
  children: React.ReactNode;
  params: Promise<{ locale: string }>;
};

export default async function DashboardLayout({
  children,
  params,
}: DashboardLayoutProps) {
  const { locale } = await params;
  setRequestLocale(locale);

  const tBrand = await getTranslations("Brand");
  const tNav = await getTranslations("Dashboard.nav");

  return (
    <div className="flex min-h-screen">
      {/* ── Sidebar ─────────────────────────────────────────────── */}
      <aside className="hidden w-64 shrink-0 border-r border-cosmos-700/60 bg-cosmos-900/40 backdrop-blur-sm lg:flex lg:flex-col">
        {/* Brand block */}
        <div className="border-b border-cosmos-700/60 px-6 py-6">
          <p className="text-[10px] uppercase tracking-[0.32em] text-sothoth-400/70">
            {tBrand("tagline")}
          </p>
          <h1 className="mt-1 font-display text-2xl tracking-wider text-sothoth-400">
            {tBrand("name")}
          </h1>
        </div>

        {/* Navigation */}
        <nav className="flex-1 space-y-1 px-3 py-6">
          <NavLink href={`/${locale}/overview`} active >
            {tNav("overview")}
          </NavLink>
          <NavLink href={`/${locale}/pools`} active>
            {tNav("pools")}
          </NavLink>
        </nav>

        {/* Footer pill — version / phase indicator */}
        <div className="border-t border-cosmos-700/60 px-6 py-4">
          <span className="inline-block rounded-full border border-sothoth-600/40 px-3 py-1 text-[10px] uppercase tracking-widest text-sothoth-400/70">
            {tNav("phaseLabel")}
          </span>
        </div>
      </aside>

      {/* ── Main content area ───────────────────────────────────── */}
      <main className="flex-1 overflow-x-hidden">{children}</main>
    </div>
  );
}

// ── Sub-components ────────────────────────────────────────────────────

function NavLink({
  href,
  active = false,
  children,
}: {
  href: string;
  active?: boolean;
  children: React.ReactNode;
}) {
  // Active state styled with a subtle sothoth-tinted background. Once
  // we have several routes, we'll resolve `active` from the current
  // pathname; for now it is passed explicitly.
  const base =
    "block rounded-md px-3 py-2 text-sm tracking-wide transition-colors";
  const variant = active
    ? "bg-sothoth-600/15 text-sothoth-400"
    : "text-slate-400 hover:bg-cosmos-700/30 hover:text-slate-200";

  return (
    <a href={href} className={`${base} ${variant}`}>
      {children}
    </a>
  );
}