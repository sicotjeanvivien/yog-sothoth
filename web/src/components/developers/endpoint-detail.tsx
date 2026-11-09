"use client";

import { useEffect, useRef, useState } from "react";
import { useTranslations } from "next-intl";

import { loadClientEnv } from "@/lib/config/client-env.schema";
import type { Endpoint, EndpointParam } from "@/lib/developers/contract";
import {
  byteSize,
  curlOf,
  missingOf,
  pageValues,
  urlOf,
  type ParamValues,
} from "@/lib/developers/request";
import { openSseStream } from "@/lib/live/sse-stream";

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

/** One request of the route, with the method the contract declares and the visitor's key, if any. */
export async function send(url: string, method: string, apiKey = ""): Promise<SendResult> {
  const started = performance.now();
  try {
    const response = await fetch(url, {
      method,
      headers: {
        Accept: "application/json",
        ...(apiKey ? { Authorization: `Bearer ${apiKey}` } : {}),
      },
      signal: AbortSignal.timeout(loadClientEnv().NEXT_PUBLIC_YOG_API_TIMEOUT_MS),
    });
    return responseResult(response.status, response.statusText, response.ok, await response.text(), started);
  } catch (err) {
    return { kind: "network", message: err instanceof Error ? err.message : String(err) };
  }
}

function responseResult(
  status: number,
  statusText: string,
  ok: boolean,
  text: string,
  started: number,
): SendResult {
  let json: unknown = null;
  try {
    json = JSON.parse(text);
  } catch {
    json = null;
  }
  return {
    kind: "response",
    status: `${status} ${statusText}`.trim(),
    ok,
    ms: Math.round(performance.now() - started),
    bytes: byteSize(text),
    at: new Date().toISOString().slice(11, 19) + " UTC",
    text: json === null ? text : JSON.stringify(json, null, 2),
    json,
  };
}

/** The ticket of a `POST /api/auth/tickets` answer, or null when it is a refusal. */
function ticketOf(result: SendResult): string | null {
  if (result.kind !== "response" || !result.ok) return null;
  const json = result.json as { ticket?: unknown } | null;
  return typeof json?.ticket === "string" ? json.ticket : null;
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

/**
 * The live tail: a ticket asked with the visitor's key, then one stream while listening, aborted on
 * stop and on unmount. ⚠️ Never reopened: a cut shows, and a refusal shows as an answer.
 */
function useStream(url: string, ticketUrl: string, apiKey: string) {
  const [state, setState] = useState<"off" | "connecting" | "on">("off");
  const [events, setEvents] = useState<StreamEvent[]>([]);
  const [refusal, setRefusal] = useState<SendResult | null>(null);
  const controllerRef = useRef<AbortController | null>(null);

  const stop = () => {
    controllerRef.current?.abort();
    controllerRef.current = null;
    setState("off");
  };

  const start = async () => {
    const controller = new AbortController();
    controllerRef.current = controller;
    setEvents([]);
    setRefusal(null);
    setState("connecting");

    const issued = await send(ticketUrl, "POST", apiKey);
    if (controller.signal.aborted) return;
    const ticket = ticketOf(issued);
    if (ticket === null) {
      setRefusal(issued);
      setState("off");
      return;
    }

    const started = performance.now();
    const end = await openSseStream(
      url,
      ticket,
      {
        onOpen: () => setState("on"),
        onEvent: (event) => {
          const entry = { at: new Date().toISOString().slice(11, 19), id: event.id ?? "", data: event.data };
          setEvents((current) => [...current, entry].slice(-12));
        },
      },
      controller.signal,
    );
    if (controller.signal.aborted) return;
    if (end.kind === "refused") {
      setRefusal(responseResult(end.status, end.statusText, false, end.body, started));
    } else if (end.kind === "unreachable") {
      setRefusal({ kind: "network", message: end.error instanceof Error ? end.error.message : String(end.error) });
    }
    controllerRef.current = null;
    setState("off");
  };

  useEffect(() => () => controllerRef.current?.abort(), []);

  return { state, events, refusal, start: () => void start(), stop };
}

/** An answer as the API gave it: its status, time and size, and its body. */
function ResponseView({ result }: { result: SendResult }) {
  const t = useTranslations("Developers");
  if (result.kind === "network") {
    return (
      <p className="mt-4 border border-dash-down/50 px-4 py-3 text-[14px] text-dash-down">
        {t("response.network", { message: result.message })}
      </p>
    );
  }
  return (
    <>
      <div className="mt-3.5 flex flex-wrap items-center gap-x-4 gap-y-1.5 text-[12px] text-dash-ink-2">
        <span className={`font-dash-mono font-semibold ${result.ok ? "text-dash-up" : "text-dash-down"}`}>
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
    </>
  );
}

export function EndpointDetail({
  endpoint,
  base,
  values,
  onValues,
  result,
  onResult,
  apiKey,
}: {
  endpoint: Endpoint;
  base: string;
  apiKey: string;
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
  const stream = useStream(url, `${base}/api/auth/tickets`, apiKey);

  const sendWith = async (next: ParamValues) => {
    onValues(next);
    setTab("response");
    setLoading(true);
    onResult(await send(urlOf(base, endpoint, next), endpoint.method, apiKey));
    setLoading(false);
  };

  const cursors = result?.kind === "response" ? cursorsOf(result.json) : null;
  const pageTo = (cursor: string, dir: "next" | "prev") =>
    sendWith(pageValues(endpoint, values, cursor, dir));

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
              onChange={(value) =>
                // ⚠️ A cursor belongs to the query it came from: another parameter drops it.
                onValues({ ...values, ...(param.name === "cursor" ? {} : { cursor: "" }), [param.name]: value })
              }
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
            text={curlOf(url, endpoint.sse, endpoint.method)}
            label={t("request.copyCurl")}
            copied={t("detail.copied")}
          />
          {endpoint.sse ? (
            <PrimaryButton
              disabled={stream.state === "off" && missing.length > 0}
              onClick={stream.state === "off" ? stream.start : stream.stop}
            >
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
          {!loading && result && <ResponseView result={result} />}
          {!loading && cursors && (
            <div className="mt-2.5 flex flex-wrap items-center gap-x-2.5 gap-y-2">
              <GhostButton disabled={!cursors.prev} onClick={() => cursors.prev && pageTo(cursors.prev, "prev")}>
                {t("response.prev")}
              </GhostButton>
              <GhostButton disabled={!cursors.next} onClick={() => cursors.next && pageTo(cursors.next, "next")}>
                {t("response.next")}
              </GhostButton>
              <span className="text-[12px] text-dash-ink-3">{t("response.cursorHint")}</span>
            </div>
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
          {stream.refusal && <ResponseView result={stream.refusal} />}
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
