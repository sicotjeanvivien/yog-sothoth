"use client";

/**
 * The sidebar's menu: renders `SIDEBAR_NAV`.
 *
 * An open entry is a `Link`. A closed entry is a plain `<span>` with
 * its lock — no `href`, no `tabindex`, no handler — so it
 * reacts neither to a click nor to the keyboard. The test in
 * `__tests__/sidebar-nav-list.test.tsx` holds that line.
 *
 * On the collapsed lg+ rail only the open entries remain, as their
 * icon: a lock without its label says nothing. The mobile drawer
 * ignores `collapsed`, so every collapsed style is `lg:`-scoped.
 */

import type { FC } from "react";
import { useTranslations } from "next-intl";

import { Link, usePathname } from "@/i18n/navigation";
import {
  LockIcon,
  OverviewIcon,
  type IconProps,
} from "@/components/shared/icon";

import type { OpenNavKey } from "./sidebar-keys";
import {
  SIDEBAR_NAV,
  type ClosedNavEntry,
  type OpenNavEntry,
  type SidebarNavEntry,
} from "./sidebar-nav";

// The collapsed rail's only content: an open entry without an icon
// does not compile.
const NAV_ICONS: Record<OpenNavKey, FC<IconProps>> = {
  overview: OverviewIcon,
};

const ROW = "flex min-h-9 items-center justify-between gap-2 px-[10px]";

type SidebarNavListProps = {
  collapsed: boolean;
  /** Called when a link is clicked — lets the shell close the drawer. */
  onNavigate: () => void;
};

export function SidebarNavList({ collapsed, onNavigate }: SidebarNavListProps) {
  const pathname = usePathname();
  const t = useTranslations("Dashboard.Sidebar.nav");

  const renderEntry = (entry: SidebarNavEntry, nested: boolean) =>
    entry.kind === "open" ? (
      <OpenEntry
        entry={entry}
        // Exact match: `usePathname` already strips the locale.
        active={pathname === entry.href}
        nested={nested}
        collapsed={collapsed}
        onNavigate={onNavigate}
      />
    ) : (
      <ClosedEntry entry={entry} nested={nested} />
    );

  return (
    <ul className="flex flex-col gap-[2px]">
      {SIDEBAR_NAV.map((item) =>
        item.kind === "group" ? (
          <li
            key={item.labelKey}
            className={collapsed ? "lg:hidden" : undefined}
          >
            <span className={`${ROW} text-[14px] text-dash-ink-2`}>
              <span>{t(item.labelKey)}</span>
              <span className="font-dash-mono text-[11px] text-dash-ink-3 tabular-nums">
                {item.entries.length}
              </span>
            </span>
            <ul className="flex flex-col gap-[2px]">
              {item.entries.map((entry) => (
                <li key={entry.key}>{renderEntry(entry, true)}</li>
              ))}
            </ul>
          </li>
        ) : (
          <li
            key={item.key}
            className={
              collapsed && item.kind === "closed" ? "lg:hidden" : undefined
            }
          >
            {renderEntry(item, false)}
          </li>
        ),
      )}
    </ul>
  );
}

function OpenEntry({
  entry,
  active,
  nested,
  collapsed,
  onNavigate,
}: {
  entry: OpenNavEntry;
  active: boolean;
  nested: boolean;
  collapsed: boolean;
  onNavigate: () => void;
}) {
  const t = useTranslations("Dashboard.Sidebar.nav");
  const label = t(entry.labelKey);
  const Icon = NAV_ICONS[entry.key];

  const state = active
    ? "bg-dash-surface text-dash-ink shadow-[inset_2px_0_0_var(--color-dash-accent)]"
    : "text-dash-ink-2 hover:bg-dash-surface hover:text-dash-ink";

  return (
    <Link
      href={entry.href}
      onClick={onNavigate}
      aria-current={active ? "page" : undefined}
      // Native tooltip on the collapsed rail: the OS-managed one never
      // overlaps the page.
      title={collapsed ? label : undefined}
      className={`${ROW} ${nested ? "pl-[22px] text-[13px]" : "text-[14px]"} transition-colors ${state} ${collapsed ? "lg:justify-center" : ""}`}
    >
      <span className={collapsed ? "lg:hidden" : undefined}>{label}</span>
      {collapsed && (
        <Icon size={18} className="hidden shrink-0 lg:block" />
      )}
    </Link>
  );
}

function ClosedEntry({
  entry,
  nested,
}: {
  entry: ClosedNavEntry;
  nested: boolean;
}) {
  const t = useTranslations("Dashboard.Sidebar.nav");

  return (
    <span
      aria-disabled="true"
      className={`${ROW} ${nested ? "pl-[22px] text-[13px]" : "text-[14px]"} cursor-default text-dash-ink-3`}
    >
      <span>{t(entry.labelKey)}</span>
      <LockIcon size={11} />
    </span>
  );
}
