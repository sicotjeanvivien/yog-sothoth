/**
 * Layout for the dashboard route group `(dashboard)`.
 *
 * Delegates the whole responsive chrome — sidebar, mobile drawer,
 * header, overlay — to `DashboardShell`. The route group does not
 * appear in the URL, so pages keep their natural paths
 * (`/[locale]/overview`, `/[locale]/pools`, …) while sharing this
 * chrome.
 *
 * This file is intentionally thin: it is a Server Component and does
 * nothing but mount the (client) shell. All interactivity and layout
 * mechanics live in `DashboardShell`.
 */

import { DashboardShell } from "@/components/dashboard/shell/dashboard-shell";

export default function DashboardLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return <DashboardShell>{children}</DashboardShell>;
}