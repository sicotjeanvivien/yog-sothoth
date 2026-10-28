"use client";

/** The routes on the left, the one chosen on the right: its parameters, its request, its answer. */

import { useState } from "react";
import { useTranslations } from "next-intl";

import { ChevronDownIcon } from "@/components/shared/icon";
import { FAMILIES, type Endpoint, type Family } from "@/lib/developers/contract";
import type { ParamValues } from "@/lib/developers/request";

import { EndpointDetail, type SendResult } from "./endpoint-detail";
import { EndpointList, shortPath } from "./endpoint-list";

export function ApiExplorer({
  endpoints,
  base,
}: {
  endpoints: readonly [Endpoint, ...Endpoint[]];
  base: string;
}) {
  const t = useTranslations("Developers.list");
  const [selected, setSelected] = useState(endpoints[0].id);
  const [values, setValues] = useState<Record<string, ParamValues>>({});
  const [results, setResults] = useState<Record<string, SendResult>>({});
  const [filter, setFilter] = useState("");
  const [closed, setClosed] = useState<ReadonlySet<Family>>(
    () => new Set(FAMILIES.filter((f) => f !== endpoints[0].family)),
  );
  const [listOpen, setListOpen] = useState(false);

  const endpoint = endpoints.find((e) => e.id === selected) ?? endpoints[0];

  return (
    <div className="grid grid-cols-1 items-start rail:grid-cols-[300px_minmax(0,1fr)] rail:border-l rail:border-dash-rule">
      <button
        type="button"
        aria-expanded={listOpen}
        aria-controls="ep-list"
        onClick={() => setListOpen((open) => !open)}
        className="flex min-h-12 w-full items-center gap-2.5 border-b border-dash-rule bg-dash-surface px-4 text-left text-dash-ink rail:hidden"
      >
        <span className="font-dash-mono text-[11px] font-medium tracking-[0.12em] text-dash-ink-3 uppercase">
          {t("toggle")}
        </span>
        <span className="min-w-0 flex-1 truncate font-dash-mono text-[12px]">
          {shortPath(endpoint.path)}
        </span>
        <ChevronDownIcon size={12} className={listOpen ? "rotate-180" : ""} />
      </button>
      <EndpointList
        endpoints={endpoints}
        selected={endpoint.id}
        onSelect={(id) => {
          setSelected(id);
          setListOpen(false);
        }}
        filter={filter}
        onFilter={setFilter}
        closed={closed}
        onClosed={setClosed}
        open={listOpen}
      />
      <EndpointDetail
        key={endpoint.id}
        endpoint={endpoint}
        base={base}
        values={values[endpoint.id] ?? {}}
        onValues={(next) => setValues((all) => ({ ...all, [endpoint.id]: next }))}
        result={results[endpoint.id]}
        onResult={(result) => setResults((all) => ({ ...all, [endpoint.id]: result }))}
      />
    </div>
  );
}
