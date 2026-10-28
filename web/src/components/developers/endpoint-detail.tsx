"use client";

import { useEffect, useRef, useState } from "react";
import { useTranslations } from "next-intl";

import type { Endpoint, EndpointParam } from "@/lib/developers/contract";
import {
  byteSize,
  curlOf,
  missingOf,
  urlOf,
  type ParamValues,
} from "@/lib/developers/request";

import { CopyButton, GhostButton, PrimaryButton } from "./buttons";
import { JsonView } from "./json-view";

export type SendResult =
  | {
      kind: "response";
      status: string;
      ok: boolean;
      ms: number;
      bytes: number;
      at: string;
      text: string;
      json: unknown;
    }
  | { kind: "network"; message: string };

/** A contract text, its `backquoted` parts set as code. */
function Prose({ text }: { text: string }) {
  return text.split(/`([^`]+)`/).map((part, i) =>
    i % 2 ? (
      <code key={i} className="font-dash-mono text-[0.92em] text-dash-ink">
        {part}
      </code>
    ) : (
      part
    ),
  );
}

const LABEL =
  "font-dash-mono text-[11px] font-medium tracking-[0.12em] text-dash-ink-3 uppercase";

async function send(url: string): Promise<SendResult> {
  const started = performance.now();
  try {
    const response = await fetch(url, { headers: { Accept: "application/json" } });
    const text = await response.text();
    let json: unknown = null;
    try {
      json = JSON.parse(text);
    } catch {
      json = null;
    }
    return {
      kind: "response",
      status: `${response.status} ${response.statusText}`.trim(),
      ok: response.ok,
      ms: Math.round(performance.now() - started),
      bytes: byteSize(text),
      at: new Date().toISOString().slice(11, 19) + " UTC",
      text: json === null ? text : JSON.stringify(json, null, 2),
      json,
    };
  } catch (err) {
    return { kind: "network", message: err instanceof Error ? err.message : String(err) };
  }
}

function sizeLabel(bytes: number): string {
  return bytes < 1024 ? `${bytes} B` : `${(bytes / 1024).toFixed(1)} KiB`;
}

/** A page's cursors, when the body is one of the API's pages. */
function cursorsOf(json: unknown): { next: string | null; prev: string | null } | null {
  if (typeof json !== "object" || json === null || !("nextCursor" in json)) return null;
  const page = json as { nextCursor?: string | null; prevCursor?: string | null };
  return { next: page.nextCursor ?? null, prev: page.prevCursor ?? null };
}

function ParamRow({
  param,
  value,
  onChange,
}: {
  param: EndpointParam;
  value: string;
  onChange: (value: string) => void;
}) {
  const t = useTranslations("Developers.params");
  let hint = param.description;
  if (!hint && param.name === "cursor") hint = t("cursor");
  else if (!hint && param.name === "limit" && param.min !== null && param.max !== null)
    hint = t("limit", { min: param.min, max: param.max });
  else if (!hint && param.in === "path")
    hint = param.name === "protocol" ? t("protocol") : t("address");
  if (param.enum && !param.required) hint = hint ? `${hint} ${t("enumDefault")}` : t("enumDefault");

  return (
    <div className="grid grid-cols-1 items-start gap-x-6 gap-y-2 border-b border-dash-rule py-3 last:border-b-0 rail:grid-cols-[200px_minmax(0,1fr)]">
      <div className="flex flex-wrap items-baseline gap-x-2 gap-y-1 rail:pt-2">
        <span className="font-dash-mono text-[14px] text-dash-ink">{param.name}</span>
        <span className="font-dash-mono text-[11px] text-dash-ink-3">
          {param.in === "path" ? t("path") : t("query")}
        </span>
        {param.required && (
          <span className="font-dash-mono text-[11px] text-dash-warn">{t("required")}</span>
        )}
      </div>
      <div className="flex min-w-0 flex-col items-start gap-1.5">
        {param.enum ? (
          <div role="group" aria-label={param.name} className="inline-flex flex-wrap border border-dash-rule">
            {param.enum.map((option) => {
              const on = value === option;
              return (
                <button
                  key={option}
                  type="button"
                  aria-pressed={on}
                  onClick={() => onChange(on && !param.required ? "" : option)}
                  className={`min-h-9 min-w-11 border-r border-dash-rule px-3 font-dash-mono text-[12px] last:border-r-0 ${
                    on ? "bg-dash-accent font-semibold text-dash-surface" : "text-dash-ink-2"
                  }`}
                >
                  {option}
                </button>
              );
            })}
          </div>
        ) : (
          <input
            type={param.type === "integer" ? "number" : "text"}
            value={value}
            min={param.min ?? undefined}
            max={param.max ?? undefined}
            placeholder={
              param.type === "integer" && param.min !== null ? `${param.min}–${param.max}` : ""
            }
            onChange={(event) => onChange(event.target.value)}
            aria-label={param.name}
            className={`min-h-9 w-full border border-dash-rule bg-dash-bg px-2.5 font-dash-mono text-[12px] text-dash-ink outline-none placeholder:text-dash-ink-3 focus:border-dash-accent ${
              param.type === "integer" ? "max-w-[120px]" : "max-w-[520px]"
            }`}
          />
        )}
        {hint && (
          <span className="max-w-[560px] text-[12px] text-dash-ink-3">
            <Prose text={hint} />
          </span>
        )}
      </div>
    </div>
  );
}

type StreamEvent = { at: string; id: string; data: string };

/** The live tail: one `EventSource` while listening, closed on stop and on unmount. */
function useStream(url: string) {
  const [state, setState] = useState<"off" | "connecting" | "on">("off");
  const [events, setEvents] = useState<StreamEvent[]>([]);
  const sourceRef = useRef<EventSource | null>(null);

  const stop = () => {
    sourceRef.current?.close();
    sourceRef.current = null;
    setState("off");
  };

  const start = () => {
    setEvents([]);
    setState("connecting");
    const source = new EventSource(url);
    sourceRef.current = source;
    source.onopen = () => setState("on");
    source.onerror = () => setState(source.readyState === EventSource.CLOSED ? "off" : "connecting");
    source.onmessage = (event) => {
      const entry = {
        at: new Date().toISOString().slice(11, 19),
        id: event.lastEventId,
        data: event.data,
      };
      setEvents((current) => [...current, entry].slice(-12));
    };
  };

  useEffect(() => () => sourceRef.current?.close(), []);

  return { state, events, start, stop };
}

export function EndpointDetail({
  endpoint,
  base,
  values,
  onValues,
  result,
  onResult,
}: {
  endpoint: Endpoint;
  base: string;
  values: ParamValues;
  onValues: (values: ParamValues) => void;
  result: SendResult | undefined;
  onResult: (result: SendResult) => void;
}) {
  const t = useTranslations("Developers");
  const [tab, setTab] = useState<"response" | "schema">("response");
  const [loading, setLoading] = useState(false);
  const url = urlOf(base, endpoint, values);
  const missing = missingOf(endpoint, values);
  const stream = useStream(url);

  const sendWith = async (next: ParamValues) => {
    onValues(next);
    setTab("response");
    setLoading(true);
    onResult(await send(urlOf(base, endpoint, next)));
    setLoading(false);
  };

  const cursors = result?.kind === "response" ? cursorsOf(result.json) : null;
  const pageTo = (cursor: string) => sendWith({ ...values, cursor });

  return (
    <section
      aria-labelledby="ep-h"
      className="min-w-0 border-dash-rule bg-dash-surface px-4 pt-5 pb-10 rail:min-h-screen rail:border-r rail:px-8 rail:pt-6 rail:pb-12"
    >
      <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
        <span className="border border-dash-up/50 px-2 py-0.5 font-dash-mono text-[14px] font-semibold text-dash-up">
          {endpoint.method}
        </span>
        <h2 id="ep-h" className="min-w-0 flex-auto font-dash-mono text-[16px] font-medium break-all text-dash-ink">
          {endpoint.path}
        </h2>
        <CopyButton small text={endpoint.path} label={t("detail.copyPath")} copied={t("detail.copied")} />
      </div>
      <div className="mt-1.5 flex flex-wrap gap-2 font-dash-mono text-[12px] text-dash-ink-3">
        <span>{endpoint.id}</span>
        <span>·</span>
        <span>{endpoint.contentType}</span>
      </div>
      {endpoint.summary && (
        <p className="mt-4 max-w-[760px] text-[14px] whitespace-pre-line text-dash-ink">
          <Prose text={endpoint.summary} />
        </p>
      )}
      {endpoint.description && (
        <p className="mt-2 max-w-[760px] text-[12px] whitespace-pre-line text-dash-ink-2">
          <Prose text={endpoint.description} />
        </p>
      )}
      {endpoint.responseDescription && (
        <p className="mt-3 max-w-[760px] text-[12px] text-dash-ink-2">
          <span className={`${LABEL} mr-1.5`}>{t("detail.response200")}</span>
          <Prose text={endpoint.responseDescription} />
        </p>
      )}

      <div className="mt-6 border-t border-dash-rule pt-4">
        <h3 className={LABEL}>{t("params.title")}</h3>
        {endpoint.params.length === 0 && (
          <p className="mt-2 text-[12px] text-dash-ink-3">{t("params.none")}</p>
        )}
        <div className="flex flex-col">
          {endpoint.params.map((param) => (
            <ParamRow
              key={param.name}
              param={param}
              value={values[param.name] ?? ""}
              onChange={(value) => onValues({ ...values, [param.name]: value })}
            />
          ))}
        </div>
      </div>

      <div className="mt-5 flex flex-col gap-3 border border-dash-rule bg-dash-bg px-4 py-3.5">
        <div className="flex min-w-0 items-baseline gap-2.5">
          <span className="flex-none font-dash-mono text-[11px] font-semibold text-dash-up">
            {endpoint.method}
          </span>
          <code className="min-w-0 font-dash-mono text-[12px] break-all text-dash-ink">{url}</code>
        </div>
        <div className="flex flex-wrap items-center justify-end gap-x-2.5 gap-y-2">
          {missing.length > 0 && (
            <span className="mr-auto text-[12px] text-dash-warn">
              {t("request.missing", { names: missing.join(", ") })}
            </span>
          )}
          <CopyButton
            text={curlOf(url, endpoint.sse)}
            label={t("request.copyCurl")}
            copied={t("detail.copied")}
          />
          {endpoint.sse ? (
            <PrimaryButton onClick={stream.state === "off" ? stream.start : stream.stop}>
              {stream.state === "off" ? t("request.listen") : t("request.stop")}
            </PrimaryButton>
          ) : (
            <PrimaryButton disabled={missing.length > 0 || loading} onClick={() => sendWith(values)}>
              {loading ? t("request.sending") : t("request.send")}
            </PrimaryButton>
          )}
        </div>
      </div>

      <div role="tablist" aria-label={t("tabs.label")} className="mt-7 flex gap-6 border-b border-dash-rule">
        {(["response", "schema"] as const).map((key) => (
          <button
            key={key}
            type="button"
            role="tab"
            aria-selected={tab === key}
            onClick={() => setTab(key)}
            className={`-mb-px min-h-10 border-b-2 text-[14px] ${
              tab === key ? "border-dash-accent text-dash-ink" : "border-transparent text-dash-ink-3"
            }`}
          >
            {t(`tabs.${key}`)}
            {key === "schema" && (
              <span className="ml-1 font-dash-mono text-dash-ink-3">{endpoint.fields.length}</span>
            )}
          </button>
        ))}
      </div>

      {tab === "response" && !endpoint.sse && (
        <>
          {loading && <Empty>{t("response.sending")}</Empty>}
          {!loading && !result && <Empty>{t("response.empty")}</Empty>}
          {!loading && result?.kind === "network" && (
            <p className="mt-4 border border-dash-down/50 px-4 py-3 text-[14px] text-dash-down">
              {t("response.network", { message: result.message })}
            </p>
          )}
          {!loading && result?.kind === "response" && (
            <>
              <div className="mt-3.5 flex flex-wrap items-center gap-x-4 gap-y-1.5 text-[12px] text-dash-ink-2">
                <span
                  className={`font-dash-mono font-semibold ${result.ok ? "text-dash-up" : "text-dash-down"}`}
                >
                  {result.status}
                </span>
                <span className="font-dash-mono">{result.ms} ms</span>
                <span className="font-dash-mono">{sizeLabel(result.bytes)}</span>
                <span className="font-dash-mono text-dash-ink-3">{result.at}</span>
                <span className="ml-auto">
                  <CopyButton small text={result.text} label={t("response.copyJson")} copied={t("detail.copied")} />
                </span>
              </div>
              <JsonView text={result.text} label={t("response.body")} />
              {cursors && (
                <div className="mt-2.5 flex flex-wrap items-center gap-x-2.5 gap-y-2">
                  <GhostButton disabled={!cursors.prev} onClick={() => cursors.prev && pageTo(cursors.prev)}>
                    {t("response.prev")}
                  </GhostButton>
                  <GhostButton disabled={!cursors.next} onClick={() => cursors.next && pageTo(cursors.next)}>
                    {t("response.next")}
                  </GhostButton>
                  <span className="text-[12px] text-dash-ink-3">{t("response.cursorHint")}</span>
                </div>
              )}
            </>
          )}
        </>
      )}

      {tab === "response" && endpoint.sse && (
        <>
          <div className="mt-3.5 flex flex-wrap items-center gap-x-4 gap-y-1.5 text-[12px] text-dash-ink-2">
            <span
              className={`font-dash-mono font-semibold ${stream.state === "on" ? "text-dash-up" : "text-dash-ink-3"}`}
            >
              {stream.state === "on"
                ? t("stream.connected")
                : stream.state === "connecting"
                  ? t("stream.connecting")
                  : t("stream.disconnected")}
            </span>
            <span className="font-dash-mono text-dash-ink-3">text/event-stream</span>
            <span className="font-dash-mono text-dash-ink-3">
              {t("stream.events", { count: stream.events.length })}
            </span>
          </div>
          <div
            role="log"
            aria-label={t("stream.log")}
            className="mt-2.5 flex min-h-[120px] flex-col gap-1.5 overflow-x-auto border border-dash-rule bg-dash-bg px-4 py-3 font-dash-mono text-[12px]"
          >
            {stream.events.length === 0 && (
              <span className="text-dash-ink-3">
                {stream.state === "off" ? t("stream.idle") : t("stream.waiting")}
              </span>
            )}
            {[...stream.events].reverse().map((event, i) => (
              <div key={`${event.id}-${i}`} className="flex min-w-0 gap-3">
                <span className="flex-none text-dash-ink-3">{event.at}</span>
                <span className="flex-none text-dash-ink-2">id: {event.id}</span>
                <span className="min-w-0 break-all text-dash-ink">{event.data}</span>
              </div>
            ))}
          </div>
        </>
      )}

      {tab === "schema" && (
        <>
          <p className="mt-3 mb-2.5 text-[12px] text-dash-ink-3">
            {t("schema.returns")}{" "}
            <span className="font-dash-mono text-dash-ink">{endpoint.schemaName}</span>
          </p>
          {endpoint.fields.length > 0 && (
            <div role="table" aria-label={t("schema.table")} className="border-t border-dash-rule">
              <div
                role="row"
                className="grid grid-cols-[minmax(0,1fr)_auto] gap-x-5 border-b border-dash-rule py-2 font-dash-mono text-[11px] tracking-[0.1em] text-dash-ink-3 uppercase rail:grid-cols-[minmax(180px,1.1fr)_minmax(120px,0.7fr)_minmax(0,2fr)]"
              >
                <div role="columnheader">{t("schema.field")}</div>
                <div role="columnheader">{t("schema.type")}</div>
                <div role="columnheader" className="hidden rail:block">
                  {t("schema.description")}
                </div>
              </div>
              {endpoint.fields.map((field) => {
                const depth = field.path.split(".").length - 1;
                const leaf = depth ? `.${field.path.split(".").at(-1)}` : field.path;
                return (
                  <div
                    key={field.path}
                    role="row"
                    className="grid grid-cols-[minmax(0,1fr)_auto] items-baseline gap-x-5 gap-y-1 border-b border-dash-rule py-2 text-[12px] rail:grid-cols-[minmax(180px,1.1fr)_minmax(120px,0.7fr)_minmax(0,2fr)]"
                  >
                    <div
                      role="cell"
                      title={field.path}
                      style={{ paddingLeft: depth * 16 }}
                      className="font-dash-mono break-all text-dash-ink"
                    >
                      {leaf}
                      {field.required && (
                        <span title={t("schema.alwaysPresent")} className="ml-0.5 text-dash-warn">
                          *
                        </span>
                      )}
                    </div>
                    <div role="cell" className="font-dash-mono text-dash-ink-3">
                      {field.type}
                    </div>
                    <div role="cell" className="col-span-full text-dash-ink-2 rail:col-span-1">
                      {field.description && <Prose text={field.description} />}
                    </div>
                  </div>
                );
              })}
            </div>
          )}
          {endpoint.fields.length > 0 && (
            <p className="mt-2.5 text-[12px] text-dash-ink-3">
              <span className="text-dash-warn">*</span> {t("schema.legend")}
            </p>
          )}
          <div className="mt-5 border border-dash-rule bg-dash-bg px-4 py-3">
            <span className={LABEL}>{t("schema.errorTitle")}</span>
            <p className="mt-1.5 text-[12px] text-dash-ink-2">{t("schema.error")}</p>
          </div>
        </>
      )}
    </section>
  );
}

function Empty({ children }: { children: React.ReactNode }) {
  return (
    <div className="mt-4 border border-dashed border-dash-rule px-4 py-7 text-[14px] text-dash-ink-3">
      {children}
    </div>
  );
}
