"use client";

import { useTranslations } from "next-intl";

import { ChevronDownIcon, SearchIcon } from "@/components/shared/icon";
import { FAMILIES, type Endpoint, type Family } from "@/lib/developers/contract";

/** The path without its `/api` prefix, as the list and the phone toggle show it. */
export function shortPath(path: string): string {
  return path.replace(/^\/api/, "");
}

function matches(e: Endpoint, query: string): boolean {
  return `${e.path} ${e.id} ${e.family}`.toLowerCase().includes(query);
}

export function EndpointList({
  endpoints,
  selected,
  onSelect,
  filter,
  onFilter,
  closed,
  onClosed,
  open,
}: {
  endpoints: readonly Endpoint[];
  selected: string;
  onSelect: (id: string) => void;
  filter: string;
  onFilter: (filter: string) => void;
  closed: ReadonlySet<Family>;
  onClosed: (closed: Set<Family>) => void;
  /** Up to 720 px the list folds behind its toggle; from the rail it always shows. */
  open: boolean;
}) {
  const t = useTranslations("Developers");
  const query = filter.toLowerCase().trim();
  const shown = endpoints.filter((e) => !query || matches(e, query));
  const anyOpen = FAMILIES.some((f) => !closed.has(f));

  return (
    <aside
      id="ep-list"
      aria-label={t("list.toggle")}
      className={`${open ? "block border-b" : "hidden"} max-h-[60vh] overflow-y-auto border-dash-rule bg-dash-surface pt-4 pb-6 rail:sticky rail:top-14 rail:block rail:max-h-[calc(100vh-56px)] rail:border-r rail:border-b-0`}
    >
      <label className="mx-4 flex items-center gap-2 border border-dash-rule bg-dash-bg px-2.5 text-dash-ink-3 focus-within:border-dash-accent">
        <SearchIcon size={16} />
        <input
          type="search"
          value={filter}
          onChange={(event) => onFilter(event.target.value)}
          placeholder={t("list.filter")}
          aria-label={t("list.filterLabel")}
          className="min-h-9 min-w-0 flex-1 bg-transparent text-[14px] text-dash-ink outline-none placeholder:text-dash-ink-3"
        />
      </label>
      <div className="mx-4 mt-2.5 mb-1.5 flex items-center justify-between text-[12px] text-dash-ink-3">
        <span>
          {query
            ? t("list.countFiltered", { shown: shown.length, count: endpoints.length })
            : t("list.count", { count: endpoints.length })}
        </span>
        <button
          type="button"
          onClick={() => onClosed(anyOpen ? new Set(FAMILIES) : new Set())}
          className="min-h-7 text-[12px] text-dash-accent-2 hover:underline"
        >
          {anyOpen ? t("list.collapseAll") : t("list.expandAll")}
        </button>
      </div>
      {shown.length === 0 && (
        <p className="mx-4 text-[12px] text-dash-ink-3">{t("list.noMatch")}</p>
      )}
      {FAMILIES.map((family) => {
        const ops = shown.filter((e) => e.family === family);
        if (ops.length === 0) return null;
        const expanded = query !== "" || !closed.has(family);
        const toggle = () => {
          const next = new Set(closed);
          if (expanded) next.add(family);
          else next.delete(family);
          onClosed(next);
        };
        return (
          <div key={family} className="border-t border-dash-rule">
            <button
              type="button"
              aria-expanded={expanded}
              onClick={query ? undefined : toggle}
              className="flex min-h-10 w-full items-center gap-2 px-4 text-left text-[14px] font-medium text-dash-ink"
            >
              <span className="flex-1">{t(`families.${family}`)}</span>
              <span className="font-dash-mono text-[11px] text-dash-ink-3">{ops.length}</span>
              <ChevronDownIcon
                size={12}
                className={`text-dash-ink-3 transition-transform ${expanded ? "rotate-180" : ""}`}
              />
            </button>
            {expanded && (
              <ul className="pb-2">
                {ops.map((e) => {
                  const on = e.id === selected;
                  return (
                    <li key={e.id}>
                      <button
                        type="button"
                        aria-current={on}
                        onClick={() => onSelect(e.id)}
                        className={`flex min-h-8 w-full items-center gap-2 px-4 py-1 text-left hover:bg-dash-surface-2 hover:text-dash-ink ${
                          on
                            ? "bg-dash-surface-2 text-dash-ink shadow-[inset_2px_0_0_var(--color-dash-accent)]"
                            : "text-dash-ink-2"
                        }`}
                      >
                        <span className="flex-none font-dash-mono text-[11px] font-semibold tracking-[0.04em] text-dash-up">
                          {e.method}
                        </span>
                        <span className="min-w-0 font-dash-mono text-[12px] break-all">
                          {shortPath(e.path)}
                        </span>
                        {e.sse && (
                          <span className="ml-auto flex-none border border-dash-rule px-1 font-dash-mono text-[11px] text-dash-ink-3">
                            SSE
                          </span>
                        )}
                      </button>
                    </li>
                  );
                })}
              </ul>
            )}
          </div>
        );
      })}
    </aside>
  );
}
