/**
 * Layout for the dashboard route group `(dashboard)`.
 *
 * Owns the persistent shell — sidebar on the left, scrollable content
 * on the right. The route group `(dashboard)` does not appear in the
 * URL, so pages under it keep their natural paths (`/[locale]/pools`,
 * `/[locale]/overview`, …) while sharing this chrome.
 *
 * # Scroll & sticky sidebar
 *
 * The sidebar is `position: sticky` (set inside the `Sidebar`
 * component). For sticky to work the scroll must belong to the page
 * (the document), not to an inner `overflow` container. This layout
 * therefore deliberately does NOT put `overflow-*` on the flex
 * wrapper: the wrapper grows with the content, the document scrolls,
 * and the sidebar sticks to the viewport top.
 *
 * `items-start` on the flex container is important — without it the
 * default `stretch` would force the sidebar to the full content
 * height, and `sticky` would have nothing to scroll against.
 *
 * The sidebar is an autonomous Client Component: it reads the route
 * and its labels itself, so it is mounted here with no props.
 */

import { Sidebar } from "@/components/dashboard/sidebar/sidebar";

type DashboardLayoutProps = {
  children: React.ReactNode;
};

export default function DashboardLayout({ children }: DashboardLayoutProps) {
  return (
    <div className="flex min-h-screen items-start">
      <Sidebar />
      <main className="min-w-0 flex-1">{children}</main>
    </div>
  );
}