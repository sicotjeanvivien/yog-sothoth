"use client";

/**
 * The page's one connection to `/api/live`: every block that streams subscribes to its topic here
 * instead of opening its own connection.
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
import { openLive, type StreamStatus } from "@/lib/live/live-connection";
import { requestLiveToken } from "@/lib/live/live-token";
import { livePath, liveScopePath, type LiveTopic } from "@/lib/live/live-topics";

type Handler = (data: unknown) => void;

type Live = {
  protocol: ProtocolResponse | undefined;
  status: StreamStatus;
  /** Receives each parsed message of `topic`; returns the unsubscribe. */
  subscribe: (topic: LiveTopic, handler: Handler) => () => void;
  /** Called at an opening that follows a loss. */
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
  const missed = useRef(false);
  const path = livePath(topics, protocol);
  const scopePath = liveScopePath(protocol);
  const topicsKey = topics.join(",");

  useEffect(() => {
    const { NEXT_PUBLIC_YOG_API_URL } = loadClientEnv();
    const opened = topicsKey.split(",") as LiveTopic[];
    const connection = openLive(
      `${NEXT_PUBLIC_YOG_API_URL}${path}`,
      opened,
      {
        onStatus: setStatus,
        onEvent: (topic, data) => handlers.current.get(topic)?.forEach((handler) => handler(data)),
        onReopen: () => reopenHandlers.current.forEach((handler) => handler()),
      },
      missed.current,
      () => requestLiveToken(scopePath, opened),
    );

    return () => {
      missed.current = connection.close();
      setStatus("connecting");
    };
  }, [path, scopePath, topicsKey]);

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
