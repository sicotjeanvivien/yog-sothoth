"use client";

/**
 * The sidebar's menu. ⚠️ A closed entry is a `<span role="link" aria-disabled>` with no `href`,
 * `tabindex` or handler. The collapsed rail keeps only the open entries' icons; every collapsed
 * style is `rail:`-scoped. The `__tests__` hold both rules.
 */

import { useId, type FC } from "react";
import { useTranslations } from "next-intl";

import { Link, usePathname } from "@/i18n/navigation";
import { OverviewIcon, type IconProps } from "@/components/shared/icon";

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
  return `flex min-h-11 rail:min-h-9 items-center justify-between gap-2 pr-[10px] ${
    nested ? "pl-[22px] text-[13px]" : "pl-[10px] text-[14px]"
  }`;
}

type SidebarNavListProps = {
  collapsed: boolean;
  /** Lets the shell close the drawer on navigation. */
  onNavigate: () => void;
};

export function SidebarNavList({ collapsed, onNavigate }: SidebarNavListProps) {
  const pathname = usePathname();
  const t = useTranslations("Dashboard.Sidebar.nav");
  const groupIdPrefix = useId();

  const hiddenWhenCollapsed = collapsed ? "rail:hidden" : undefined;

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
        <ClosedEntry label={t(entry.labelKey)} soon={t("soon")} nested={nested} />
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
      className={`${rowClass(nested)} transition-colors focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-dash-accent ${state} ${collapsed ? "rail:justify-center rail:px-0" : ""}`}
    >
      <span className={collapsed ? "rail:sr-only" : undefined}>{label}</span>
      {collapsed && (
        <Icon size={18} className="hidden shrink-0 rail:block" />
      )}
    </Link>
  );
}

function ClosedEntry({
  label,
  soon,
  nested,
}: {
  label: string;
  soon: string;
  nested: boolean;
}) {
  return (
    <span
      role="link"
      aria-disabled="true"
      className={`${rowClass(nested)} cursor-default text-dash-ink-3`}
    >
      <span>{label}</span>
      <span className="border border-dashed border-dash-ink-3 px-1 py-px font-dash-mono text-[11px] whitespace-nowrap uppercase">
        {soon}
      </span>
    </span>
  );
}
