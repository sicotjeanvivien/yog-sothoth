"use client";

/**
 * The sidebar's menu. ⚠️ A closed entry is a `<span role="link" aria-disabled>` with no `href`,
 * `tabindex` or handler. The collapsed rail keeps every row as its icon alone and hides the
 * platforms' products; every collapsed style is `rail:`-scoped. The `__tests__` hold these rules.
 */

import { useId, type FC } from "react";
import { useTranslations } from "next-intl";

import { Link, usePathname } from "@/i18n/navigation";
import {
  OverviewIcon,
  TokenIcon,
  type IconProps,
} from "@/components/shared/icon";

import type { IconKey } from "./sidebar-keys";
import {
  SIDEBAR_NAV,
  type OpenNavEntry,
  type ProductEntry,
  type SidebarNavEntry,
  type SidebarNavPlatform,
} from "./sidebar-nav";

type NavIcon = { kind: "icon"; Icon: FC<IconProps> } | { kind: "letter"; text: string };

const NAV_ICONS: Record<IconKey, NavIcon> = {
  overview: { kind: "icon", Icon: OverviewIcon },
  meteora: { kind: "letter", text: "M" },
  raydium: { kind: "letter", text: "R" },
  orca: { kind: "letter", text: "O" },
  token: { kind: "icon", Icon: TokenIcon },
};

/** The 20 px box of a menu icon, shared with the Preferences. */
export const ICON_BOX = "inline-flex h-5 w-5 shrink-0 items-center justify-center";

/** A product sits under its platform's label, without an icon. */
function rowClass(collapsed: boolean, product = false): string {
  return `flex min-h-11 rail:min-h-9 items-center justify-between gap-2 pr-[10px] text-[14px] ${
    product ? "pl-10" : "pl-[10px]"
  } ${collapsed ? "rail:justify-center rail:px-0" : ""}`;
}

type SidebarNavListProps = {
  collapsed: boolean;
  /** Lets the shell close the drawer on navigation. */
  onNavigate: () => void;
};

export function SidebarNavList({ collapsed, onNavigate }: SidebarNavListProps) {
  const pathname = usePathname();
  const t = useTranslations("Dashboard.Sidebar.nav");
  const idPrefix = useId();

  const renderEntry = (entry: SidebarNavEntry | ProductEntry, icon: NavIcon | null) => {
    const label = t(entry.labelKey);
    return entry.kind === "open" ? (
      <OpenEntry
        entry={entry}
        label={label}
        icon={icon}
        // Exact match: `usePathname` already strips the locale.
        active={pathname === entry.href}
        collapsed={collapsed}
        onNavigate={onNavigate}
      />
    ) : (
      <ClosedEntry
        label={label}
        icon={icon}
        soon={t("soon")}
        soonTitle={t("soonTitle", { label })}
        collapsed={collapsed}
      />
    );
  };

  return (
    <ul className="flex flex-col gap-[2px]">
      {SIDEBAR_NAV.map((item, index) => {
        if (item.kind !== "platform") {
          return <li key={item.key}>{renderEntry(item, NAV_ICONS[item.key])}</li>;
        }
        const first = SIDEBAR_NAV[index - 1]?.kind !== "platform";
        return (
          // The first platform stands apart from the entries above it.
          <li key={item.key} className={first ? "mt-2.5" : undefined}>
            <Platform
              platform={item}
              captionId={`${idPrefix}-${item.key}`}
              pathname={pathname}
              collapsed={collapsed}
              onNavigate={onNavigate}
              renderEntry={renderEntry}
            />
          </li>
        );
      })}
    </ul>
  );
}

/** A platform's row, then its products, which the collapsed rail hides. */
function Platform({
  platform,
  captionId,
  pathname,
  collapsed,
  onNavigate,
  renderEntry,
}: {
  platform: SidebarNavPlatform;
  captionId: string;
  pathname: string;
  collapsed: boolean;
  onNavigate: () => void;
  renderEntry: (entry: SidebarNavEntry | ProductEntry, icon: NavIcon | null) => React.ReactNode;
}) {
  const t = useTranslations("Dashboard.Sidebar.nav");
  const label = t(platform.labelKey);
  const icon = NAV_ICONS[platform.key];

  if (platform.products.length === 0) {
    return (
      <ClosedEntry
        label={label}
        icon={icon}
        soon={t("soon")}
        soonTitle={t("soonTitle", { label })}
        collapsed={collapsed}
      />
    );
  }

  const target = platform.products.find((product) => product.kind === "open");
  const current = platform.products.some(
    (product) => product.kind === "open" && product.href === pathname,
  );
  const content = <EntryLabel icon={icon} label={label} collapsed={collapsed} />;

  return (
    <>
      {target ? (
        <Link
          id={captionId}
          href={target.href}
          onClick={onNavigate}
          title={collapsed ? label : undefined}
          className={`${rowClass(collapsed)} text-dash-ink transition-colors hover:bg-dash-surface focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-dash-accent ${
            current && collapsed
              ? "rail:bg-dash-surface rail:shadow-[inset_2px_0_0_var(--color-dash-accent)]"
              : ""
          }`}
        >
          {content}
        </Link>
      ) : (
        <span
          id={captionId}
          title={collapsed ? label : undefined}
          className={`${rowClass(collapsed)} text-dash-ink`}
        >
          {content}
        </span>
      )}
      <ul
        aria-labelledby={captionId}
        className={`flex flex-col gap-[2px] ${collapsed ? "rail:hidden" : ""}`}
      >
        {platform.products.map((product) => (
          <li key={product.key}>{renderEntry(product, null)}</li>
        ))}
      </ul>
    </>
  );
}

/** The row's icon: a drawn one, or the platform's letter until its official logo. */
function EntryIcon({ icon, tone }: { icon: NavIcon; tone: string }) {
  return icon.kind === "icon" ? (
    <span aria-hidden="true" className={`${ICON_BOX} ${tone}`}>
      <icon.Icon size={16} />
    </span>
  ) : (
    <span
      aria-hidden="true"
      className={`${ICON_BOX} border border-dashed border-dash-ink-3 font-dash-mono text-[10px] text-dash-ink-2`}
    >
      {icon.text}
    </span>
  );
}

/** The icon, if any, then the label, which the collapsed rail keeps for screen readers only. */
function EntryLabel({
  icon,
  label,
  collapsed,
  tone = "",
}: {
  icon: NavIcon | null;
  label: string;
  collapsed: boolean;
  /** The icon's colour, when it is not the row's. */
  tone?: string;
}) {
  return (
    <span className="flex min-w-0 items-center gap-[10px]">
      {icon && <EntryIcon icon={icon} tone={tone} />}
      <span className={collapsed ? "rail:sr-only" : undefined}>{label}</span>
    </span>
  );
}

function OpenEntry({
  entry,
  label,
  icon,
  active,
  collapsed,
  onNavigate,
}: {
  entry: OpenNavEntry<string>;
  label: string;
  icon: NavIcon | null;
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
      className={`group ${rowClass(collapsed, icon === null)} transition-colors focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-dash-accent ${state}`}
    >
      <EntryLabel
        icon={icon}
        label={label}
        collapsed={collapsed}
        tone={active ? "" : "text-dash-ink-3 group-hover:text-dash-ink"}
      />
    </Link>
  );
}

function ClosedEntry({
  label,
  icon,
  soon,
  soonTitle,
  collapsed,
}: {
  label: string;
  icon: NavIcon | null;
  soon: string;
  soonTitle: string;
  collapsed: boolean;
}) {
  return (
    <span
      role="link"
      aria-disabled="true"
      title={collapsed ? soonTitle : undefined}
      className={`${rowClass(collapsed, icon === null)} cursor-default text-dash-ink-3`}
    >
      <EntryLabel icon={icon} label={label} collapsed={collapsed} />
      <span
        className={`border border-dashed border-dash-ink-3 px-1 py-px font-dash-mono text-[11px] whitespace-nowrap uppercase ${collapsed ? "rail:sr-only" : ""}`}
      >
        {soon}
      </span>
    </span>
  );
}
