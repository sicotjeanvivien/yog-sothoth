"use client";

/**
 * The pools table's signal cell: the worst severity, the list on hover or focus, the Alerts tab
 * on activation. ⚠️ A `role="link"` span, since the row is already a `<Link>`; hover, not click,
 * reveals the list, as click opens the tab. Labels come resolved from the server parent.
 */

import { useState } from "react";

import { useRouter } from "@/i18n/navigation";
import type { SeverityResponse } from "@/lib/api/schema/zod.gen";
import {
  SEVERITY_COLOR,
  SEVERITY_ICON,
} from "@/components/dashboard/signals/signal-display";

export type PoolSignalItem = {
  severity: SeverityResponse;
  /** Localized detector tag (raw detector name when unknown). */
  label: string;
};

export function PoolSignalsCell({
  alertsHref,
  ariaLabel,
  title,
  worst,
  items,
}: {
  /** Locale-relative href of the pool's Alerts tab. */
  alertsHref: string;
  /** Accessible name — carries the count and the destination. */
  ariaLabel: string;
  title: string;
  worst: SeverityResponse;
  items: PoolSignalItem[];
}) {
  const [open, setOpen] = useState(false);
  const router = useRouter();
  const WorstIcon = SEVERITY_ICON[worst];

  const navigate = () => router.push(alertsHref);

  return (
    <span
      className="relative inline-flex"
      onMouseEnter={() => setOpen(true)}
      onMouseLeave={() => setOpen(false)}
    >
      <span
        role="link"
        tabIndex={0}
        aria-label={ariaLabel}
        onClick={(event) => {
          // Keep the row's own <Link> out of it: this click means
          // "Alerts tab", not "pool page".
          event.preventDefault();
          event.stopPropagation();
          navigate();
        }}
        onKeyDown={(event) => {
          if (event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            event.stopPropagation();
            navigate();
          }
        }}
        onFocus={() => setOpen(true)}
        onBlur={() => setOpen(false)}
        className={`inline-flex cursor-pointer items-center rounded-[4px] p-1 transition-transform hover:scale-110 focus-visible:outline focus-visible:outline-2 focus-visible:outline-sothoth-400 ${SEVERITY_COLOR[worst]}`}
      >
        <WorstIcon size={16} />
      </span>

      {open && items.length > 0 && (
        <div
          role="note"
          className="absolute top-full left-0 z-20 mt-1 w-max max-w-[36ch] rounded-[8px] border border-sothoth-500/25 bg-cosmos-800 px-4 py-3 shadow-[0_8px_24px_rgba(0,0,0,0.45)]"
        >
          <p className="mb-2 text-[11px] font-semibold tracking-[0.2em] text-slate-500 uppercase">
            {title}
          </p>
          <ul className="flex flex-col gap-1.5">
            {items.map((item, index) => {
              const ItemIcon = SEVERITY_ICON[item.severity];
              return (
                <li key={index} className="flex items-center gap-2">
                  <span
                    className={`inline-flex ${SEVERITY_COLOR[item.severity]}`}
                  >
                    <ItemIcon size={13} />
                  </span>
                  <span className="font-mono text-[12px] text-slate-300">
                    {item.label}
                  </span>
                </li>
              );
            })}
          </ul>
        </div>
      )}
    </span>
  );
}
