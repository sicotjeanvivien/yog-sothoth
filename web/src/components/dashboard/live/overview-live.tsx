"use client";

/**
 * The live connection of a page that has the advanced mode: its topics follow the mode, so the
 * activity table's collections are polled only while it shows.
 */

import type { ReactNode } from "react";

import { useOverviewMode } from "@/components/dashboard/overview/overview-mode";
import type { ProtocolResponse } from "@/lib/api/schema/zod.gen";
import { pageTopics } from "@/lib/live/live-topics";

import { LiveProvider } from "./live-provider";

export function OverviewLive({
  protocol,
  children,
}: {
  protocol?: ProtocolResponse | undefined;
  children: ReactNode;
}) {
  const { advanced } = useOverviewMode();
  return (
    <LiveProvider topics={pageTopics(advanced)} protocol={protocol}>
      {children}
    </LiveProvider>
  );
}
