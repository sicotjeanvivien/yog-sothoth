"use client";

/**
 * The row of a protocol page's tabs: links that change `?tab=` and keep the period, and a digit
 * key opens its tab. The page renders it once a tab has shipped.
 */

import { useEffect } from "react";

import type { PagePath } from "@/config/site";
import { Link, useRouter } from "@/i18n/navigation";
import { pageQuery, type Period } from "@/lib/overview/period";
import { tabForKey, tabNumber, tabQuery, type ProtocolTab } from "@/lib/protocol/tabs";

/**
 * ⚠️ Shift is allowed: AZERTY types its top-row digits with it. A key typed into a field,
 * held down, composed, or with another modifier belongs to that field or to the browser.
 */
function isShortcut(event: KeyboardEvent): boolean {
  if (event.altKey || event.ctrlKey || event.metaKey || event.repeat || event.isComposing) {
    return false;
  }
  const target = event.target;
  return !(
    target instanceof HTMLElement &&
    (target.isContentEditable || ["INPUT", "SELECT", "TEXTAREA"].includes(target.tagName))
  );
}

export function ProtocolTabs({
  pathname,
  period,
  current,
  tabs,
  label,
}: {
  pathname: PagePath;
  period: Period;
  current: ProtocolTab;
  /** The ready tabs, in their row's order, with their label. */
  tabs: readonly { key: ProtocolTab; label: string }[];
  label: string;
}) {
  const router = useRouter();

  useEffect(() => {
    const ready = new Set(tabs.map((t) => t.key));
    const onKey = (event: KeyboardEvent) => {
      const tab = isShortcut(event) ? tabForKey(event.key, ready) : null;
      if (tab !== null && tab !== current) {
        router.push(
          { pathname, query: pageQuery(period, tabQuery(tab, ready)) },
          { scroll: false },
        );
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [tabs, current, pathname, period, router]);

  const ready = new Set(tabs.map((t) => t.key));

  return (
    <nav
      aria-label={label}
      className="flex gap-x-9 overflow-x-auto border-b border-l border-dash-rule bg-dash-bg px-4 [mask-image:linear-gradient(to_right,#000_calc(100%-40px),transparent)] [scrollbar-width:none] rail:px-6 [&::-webkit-scrollbar]:hidden"
    >
      {tabs.map((tab) => {
        const on = tab.key === current;
        return (
          <Link
            key={tab.key}
            href={{ pathname, query: pageQuery(period, tabQuery(tab.key, ready)) }}
            scroll={false}
            aria-current={on ? "page" : undefined}
            className={`inline-flex shrink-0 items-baseline gap-2 pt-5 pb-4 text-[16px] font-medium tracking-[-0.01em] ${
              on
                ? "text-dash-ink shadow-[inset_0_-2px_0_var(--color-dash-accent)]"
                : "text-dash-ink-2 hover:text-dash-ink"
            }`}
          >
            <span
              className={`font-dash-mono text-[11px] font-medium tracking-[0.06em] ${
                on ? "text-dash-accent" : "text-dash-ink-3"
              }`}
            >
              {tabNumber(tab.key)}
            </span>
            {tab.label}
          </Link>
        );
      })}
    </nav>
  );
}
