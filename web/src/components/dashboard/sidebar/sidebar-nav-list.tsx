"use client";

/**
 * The sidebar's menu: renders `SIDEBAR_NAV`.
 *
 * An open entry is a `Link`. A closed entry is a disabled link — a
 * `<span role="link" aria-disabled>`, the pagination's convention — with
 * its lock: no `href`, no `tabindex`, no handler, so it reacts neither
 * to a click nor to the keyboard. `__tests__/sidebar-nav-list.test.tsx`
 * holds that line.
 *
 * On the collapsed lg+ rail only the open entries remain, as their
 * centred icon — wherever they sit, in a group or not; a group shows
 * only if it holds one. A lock without its label says nothing. The
 * mobile drawer ignores `collapsed`, so every collapsed style is
 * `lg:`-scoped; `__tests__/sidebar-nav-list-collapsed.test.tsx` holds
 * those rules.
 */

import { useId, type FC } from "react";
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
  type OpenNavEntry,
  type SidebarNavEntry,
} from "./sidebar-nav";

// The collapsed rail's only content: an open entry without an icon
// does not compile.
const NAV_ICONS: Record<OpenNavKey, FC<IconProps>> = {
  overview: OverviewIcon,
};

function rowClass(nested: boolean): string {
  return `flex min-h-9 items-center justify-between gap-2 pr-[10px] ${
    nested ? "pl-[22px] text-[13px]" : "pl-[10px] text-[14px]"
  }`;
}

type SidebarNavListProps = {
  collapsed: boolean;
  /** Called when a link is clicked — lets the shell close the drawer. */
  onNavigate: () => void;
};

export function SidebarNavList({ collapsed, onNavigate }: SidebarNavListProps) {
  const pathname = usePathname();
  const t = useTranslations("Dashboard.Sidebar.nav");
  const groupIdPrefix = useId();

  const hiddenWhenCollapsed = collapsed ? "lg:hidden" : undefined;

  const renderEntry = (entry: SidebarNavEntry, nested: boolean) =>
    entry.kind === "open" ? (
      <li key={entry.key}>
        <OpenEntry
          entry={entry}
          label={t(entry.labelKey)}
          // Exact match: `usePathname` already strips the locale.
          active={pathname === entry.href}
          nested={nested}
          collapsed={collapsed}
          onNavigate={onNavigate}
        />
      </li>
    ) : (
      <li key={entry.key} className={hiddenWhenCollapsed}>
        <ClosedEntry label={t(entry.labelKey)} nested={nested} />
      </li>
    );

  return (
    <ul className="flex flex-col gap-[2px]">
      {SIDEBAR_NAV.map((item) => {
        if (item.kind !== "group") return renderEntry(item, false);
        const captionId = `${groupIdPrefix}-${item.labelKey}`;
        const holdsOpen = item.entries.some((entry) => entry.kind === "open");
        return (
          <li
            key={item.labelKey}
            className={holdsOpen ? undefined : hiddenWhenCollapsed}
          >
            <span
              id={captionId}
              className={`${rowClass(false)} text-dash-ink-2 ${hiddenWhenCollapsed ?? ""}`}
            >
              <span>{t(item.labelKey)}</span>
              <span
                aria-hidden="true"
                className="font-dash-mono text-[11px] text-dash-ink-3 tabular-nums"
              >
                {item.entries.length}
              </span>
            </span>
            <ul
              aria-labelledby={captionId}
              className="flex flex-col gap-[2px]"
            >
              {item.entries.map((entry) => renderEntry(entry, true))}
            </ul>
          </li>
        );
      })}
    </ul>
  );
}

function OpenEntry({
  entry,
  label,
  active,
  nested,
  collapsed,
  onNavigate,
}: {
  entry: OpenNavEntry;
  label: string;
  active: boolean;
  nested: boolean;
  collapsed: boolean;
  onNavigate: () => void;
}) {
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
      // Inset outline: the scrolling nav would clip one drawn outside.
      className={`${rowClass(nested)} transition-colors focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-dash-accent ${state} ${collapsed ? "lg:justify-center lg:px-0" : ""}`}
    >
      <span className={collapsed ? "lg:sr-only" : undefined}>{label}</span>
      {collapsed && (
        <Icon size={18} className="hidden shrink-0 lg:block" />
      )}
    </Link>
  );
}

function ClosedEntry({ label, nested }: { label: string; nested: boolean }) {
  return (
    <span
      role="link"
      aria-disabled="true"
      className={`${rowClass(nested)} cursor-default text-dash-ink-3`}
    >
      <span>{label}</span>
      <LockIcon size={11} />
    </span>
  );
}
