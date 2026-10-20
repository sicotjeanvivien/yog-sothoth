"use client";

/**
 * The sidebar's menu. ⚠️ A closed entry is a `<span role="link" aria-disabled>` with no `href`,
 * `tabindex` or handler. The collapsed rail keeps every entry as its icon alone; every collapsed
 * style is `rail:`-scoped. The `__tests__` hold both rules.
 */

import { useId, type FC } from "react";
import { useTranslations } from "next-intl";

import { Link, usePathname } from "@/i18n/navigation";
import {
  OverviewIcon,
  TokenIcon,
  type IconProps,
} from "@/components/shared/icon";

import type { NavKey } from "./sidebar-keys";
import {
  SIDEBAR_NAV,
  type ClosedNavEntry,
  type OpenNavEntry,
  type SidebarNavEntry,
} from "./sidebar-nav";

type NavIcon = { kind: "icon"; Icon: FC<IconProps> } | { kind: "abbr"; text: string };

// An entry without an icon does not compile: the collapsed rail shows nothing else.
const NAV_ICONS: Record<NavKey, NavIcon> = {
  overview: { kind: "icon", Icon: OverviewIcon },
  meteoraDammV2: { kind: "abbr", text: "D2" },
  meteoraDlmm: { kind: "abbr", text: "DL" },
  raydium: { kind: "abbr", text: "RY" },
  orca: { kind: "abbr", text: "OR" },
  token: { kind: "icon", Icon: TokenIcon },
};

function rowClass(collapsed: boolean): string {
  return `flex min-h-11 rail:min-h-9 items-center justify-between gap-2 px-[10px] text-[14px] ${
    collapsed ? "rail:justify-center rail:px-0" : ""
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

  const renderEntry = (entry: SidebarNavEntry) => (
    <li key={entry.key}>
      {entry.kind === "open" ? (
        <OpenEntry
          entry={entry}
          label={t(entry.labelKey)}
          // Exact match: `usePathname` already strips the locale.
          active={pathname === entry.href}
          collapsed={collapsed}
          onNavigate={onNavigate}
        />
      ) : (
        <ClosedEntry
          entry={entry}
          label={t(entry.labelKey)}
          soon={t("soon")}
          collapsed={collapsed}
        />
      )}
    </li>
  );

  return (
    <ul className="flex flex-col gap-[2px]">
      {SIDEBAR_NAV.map((item) => {
        if (item.kind !== "group") return renderEntry(item);
        const captionId = `${groupIdPrefix}-${item.labelKey}`;
        return (
          <li key={item.labelKey}>
            <span
              id={captionId}
              className={`${rowClass(false)} text-dash-ink-2 ${collapsed ? "rail:hidden" : ""}`}
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
              {item.entries.map(renderEntry)}
            </ul>
          </li>
        );
      })}
    </ul>
  );
}

/** The entry's icon, or its protocol's abbreviation, in a 20 px box. */
function EntryIcon({ entryKey, tone }: { entryKey: NavKey; tone: string }) {
  const icon = NAV_ICONS[entryKey];
  const box = `inline-flex h-5 w-5 shrink-0 items-center justify-center ${tone}`;

  return icon.kind === "icon" ? (
    <span aria-hidden="true" className={box}>
      <icon.Icon size={16} />
    </span>
  ) : (
    <span
      aria-hidden="true"
      className={`${box} border border-dash-rule font-dash-mono text-[9px] tracking-[0.02em]`}
    >
      {icon.text}
    </span>
  );
}

/** The icon then the label, which the collapsed rail keeps for screen readers only. */
function EntryLabel({
  entryKey,
  label,
  collapsed,
  tone = "",
}: {
  entryKey: NavKey;
  label: string;
  collapsed: boolean;
  /** The icon's colour, when it is not the row's. */
  tone?: string;
}) {
  return (
    <span className="flex min-w-0 items-center gap-[10px]">
      <EntryIcon entryKey={entryKey} tone={tone} />
      <span className={collapsed ? "rail:sr-only" : undefined}>{label}</span>
    </span>
  );
}

function OpenEntry({
  entry,
  label,
  active,
  collapsed,
  onNavigate,
}: {
  entry: OpenNavEntry;
  label: string;
  active: boolean;
  collapsed: boolean;
  onNavigate: () => void;
}) {
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
      className={`group ${rowClass(collapsed)} transition-colors focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-dash-accent ${state}`}
    >
      <EntryLabel
        entryKey={entry.key}
        label={label}
        collapsed={collapsed}
        tone={active ? "" : "text-dash-ink-3 group-hover:text-dash-ink"}
      />
    </Link>
  );
}

function ClosedEntry({
  entry,
  label,
  soon,
  collapsed,
}: {
  entry: ClosedNavEntry;
  label: string;
  soon: string;
  collapsed: boolean;
}) {
  return (
    <span
      role="link"
      aria-disabled="true"
      title={collapsed ? `${label} — ${soon}` : undefined}
      className={`${rowClass(collapsed)} cursor-default text-dash-ink-3`}
    >
      <EntryLabel entryKey={entry.key} label={label} collapsed={collapsed} />
      <span
        className={`border border-dashed border-dash-ink-3 px-1 py-px font-dash-mono text-[11px] whitespace-nowrap uppercase ${collapsed ? "rail:hidden" : ""}`}
      >
        {soon}
      </span>
    </span>
  );
}
