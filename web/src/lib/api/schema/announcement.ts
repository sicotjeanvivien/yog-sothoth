/**
 * Schema for the `GET /api/announcements/active` response.
 *
 * Mirrors the `AnnouncementResponse` DTO emitted by yog-api: one
 * operator announcement per entry, most severe first then most recent
 * (the ordering is the API contract — the client just takes the head).
 *
 * Notes:
 *   - `kind` and `severity` are closed enums (CHECK-constrained in
 *     the DB) — `z.enum` surfaces drift as a validation failure.
 *   - `severity` is the *announcement* scale, deliberately distinct
 *     from the signal severity concept even though the tags coincide
 *     today (editorial display choice vs detector conclusion).
 *   - `linkUrl` is a plain string, not `z.url()`: the main use case is
 *     a relative in-app target like `/changelog#v0.1.1`.
 */

import * as z from "zod";

import { Rfc3339 } from "./shared";

/** What an announcement is about — drives the label chip. */
export const AnnouncementKindSchema = z.enum([
  "maintenance",
  "incident",
  "release",
  "beta",
]);

/** Announcement kind — `"maintenance" | "incident" | "release" | "beta"`. */
export type AnnouncementKind = z.infer<typeof AnnouncementKindSchema>;

/** How prominently to display it — drives the banner styling. */
export const AnnouncementSeveritySchema = z.enum([
  "info",
  "warning",
  "critical",
]);

/** Announcement severity — `"info" | "warning" | "critical"`. */
export type AnnouncementSeverity = z.infer<typeof AnnouncementSeveritySchema>;

/** Schema for one active announcement. */
export const AnnouncementSchema = z.object({
  // Storage identity — the dismiss-cookie key.
  id: z.number(),
  kind: AnnouncementKindSchema,
  severity: AnnouncementSeveritySchema,
  // Free operator text (English, v1 decision).
  message: z.string(),
  // Optional target; usually a relative path (see file header).
  linkUrl: z.string().nullable(),
  startsAt: Rfc3339,
  // null = open-ended (shown until the operator closes it).
  endsAt: Rfc3339.nullable(),
});

/** Validated announcement payload. */
export type AnnouncementResponse = z.infer<typeof AnnouncementSchema>;

/** The endpoint returns the full active set as a bare array. */
export const AnnouncementListSchema = z.array(AnnouncementSchema);
