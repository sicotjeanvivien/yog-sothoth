/**
 * Layout for the dashboard route group `(dashboard)`.
 *
 * Owns the persistent shell — sidebar on the left, main content on
 * the right. The route group `(dashboard)` does not appear in the URL,
 * so pages under it keep their natural paths (`/[locale]/pools`,
 * `/[locale]/overview`, etc.) while sharing this chrome.
 *
 * The marketing routes live in a sibling group `(marketing)` and
 * use their own layout — no sidebar, marketing-style chrome instead.
 *
 * # Active nav item
 *
 * The sidebar receives `activeKey={null}` here. The layout does not
 * know which route is currently rendering — each page is expected
 * to provide its own active state by a mechanism we will pick when
 * the first page lands (Context, route segment introspection, or
 * a small client component reading `usePathname`). Leaving the
 * decision open in this commit avoids locking in a pattern before
 * we know what is most ergonomic.
 */

import { getTranslations } from "next-intl/server";

import { Sidebar } from "@/components/dashboard/sidebar/sidebar";

type DashboardLayoutProps = {
  children: React.ReactNode;
  params: Promise<{ locale: string }>;
};

export default async function DashboardLayout({
  children,
  params,
}: DashboardLayoutProps) {
  return (
    <div className="flex min-h-screen">
      <Sidebar />
      <main className="flex-1 overflow-x-hidden">{children}</main>
    </div>
  );
}
