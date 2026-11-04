"use client";

/**
 * The page's one connection to `/api/live`: every block that streams subscribes to its topic here
 * instead of opening its own `EventSource`. ⚠️ Nothing is replayed: after a failure or a new set
 * of topics, the next opening leaves a gap, and `onReopen` fires.
 */

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";

import type { ProtocolResponse } from "@/lib/api/schema/zod.gen";
import { loadClientEnv } from "@/lib/config/client-env.schema";
import { livePath, type LiveTopic } from "@/lib/live/live-topics";

export type StreamStatus = "connecting" | "live" | "reconnecting";

type Handler = (data: unknown) => void;

type Live = {
  protocol: ProtocolResponse | undefined;
  status: StreamStatus;
  /** Receives each parsed message of `topic`; returns the unsubscribe. */
  subscribe: (topic: LiveTopic, handler: Handler) => () => void;
  /** Called at each opening after the first: the messages in between are lost. */
  onReopen: (handler: () => void) => () => void;
};

const LiveContext = createContext<Live | null>(null);

export function LiveProvider({
  topics,
  protocol,
  children,
}: {
  topics: readonly LiveTopic[];
  protocol?: ProtocolResponse | undefined;
  children: ReactNode;
}) {
  const [status, setStatus] = useState<StreamStatus>("connecting");
  const handlers = useRef(new Map<LiveTopic, Set<Handler>>());
  const reopenHandlers = useRef(new Set<() => void>());
  // ⚠️ A gap too: a failure before any opening, after the server rendered the first page.
  const missed = useRef(false);
  const path = livePath(topics, protocol);
  const topicsKey = topics.join(",");

  useEffect(() => {
    const { NEXT_PUBLIC_YOG_API_URL } = loadClientEnv();
    const source = new EventSource(`${NEXT_PUBLIC_YOG_API_URL}${path}`);
    let opened = false;

    source.onopen = () => {
      setStatus("live");
      if (missed.current) {
        reopenHandlers.current.forEach((handler) => handler());
      }
      missed.current = false;
      opened = true;
    };

    source.onerror = () => {
      setStatus("reconnecting");
      missed.current = true;
    };

    for (const topic of new Set(topicsKey.split(",") as LiveTopic[])) {
      // ⚠️ A named event: `onmessage` only receives the unnamed ones.
      source.addEventListener(topic, (event) => {
        let data: unknown;
        try {
          data = JSON.parse((event as MessageEvent<string>).data);
        } catch {
          console.warn(`live ${topic}: unparseable event skipped`);
          return;
        }
        handlers.current.get(topic)?.forEach((handler) => handler(data));
      });
    }

    return () => {
      source.close();
      setStatus("connecting");
      missed.current ||= opened;
    };
  }, [path, topicsKey]);

  const subscribe = useCallback((topic: LiveTopic, handler: Handler) => {
    const set = handlers.current.get(topic) ?? new Set<Handler>();
    set.add(handler);
    handlers.current.set(topic, set);
    return () => {
      set.delete(handler);
    };
  }, []);

  const onReopen = useCallback((handler: () => void) => {
    reopenHandlers.current.add(handler);
    return () => {
      reopenHandlers.current.delete(handler);
    };
  }, []);

  const live = useMemo(
    () => ({ protocol, status, subscribe, onReopen }),
    [protocol, status, subscribe, onReopen],
  );

  return <LiveContext.Provider value={live}>{children}</LiveContext.Provider>;
}

export function useLive(): Live {
  const live = useContext(LiveContext);
  if (live === null) {
    throw new Error("useLive outside LiveProvider");
  }
  return live;
}
