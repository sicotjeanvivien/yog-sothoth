"use client";

/**
 * Operator announcement banner — one announcement at a time, at the
 * top of the dashboard content area.
 *
 * The Server Component side (the dashboard layout) fetches the active
 * set, reads the dismiss cookie and picks what to show
 * (`pickAnnouncement`), so the first paint is already correct — the
 * `sidebar-state` cookie pattern. This client island only owns the
 * dismiss interaction: append the id to the cookie, hide locally.
 *
 * Severity styling is the banner's own mapping. It deliberately does
 * NOT import the signals' `signal-display` vocabulary: an announcement
 * severity is an editorial display choice, not a detector conclusion —
 * only the underlying color *tokens* (sky/amber/rose) are shared, as
 * design-system consistency.
 */

import { useState } from "react";
import { useTranslations } from "next-intl";

import { Link } from "@/i18n/navigation";
import type {
  AnnouncementResponse,
  AnnouncementSeverity,
} from "@/lib/api/schema/announcement";
import {
  ANNOUNCEMENTS_COOKIE_MAX_AGE_S,
  ANNOUNCEMENTS_DISMISSED_COOKIE,
  serializeDismissedIds,
} from "@/lib/announcements/announcement-state";
import {
  AlertOctagonIcon,
  AlertTriangleIcon,
  CloseIcon,
  InfoIcon,
  type IconProps,
} from "@/components/shared/icon";

// Same rung logic as everywhere severity is displayed: shape + tint,
// never hue alone; info stays close to neutral.
const BANNER_STYLE: Record<AnnouncementSeverity, string> = {
  info: "border-sothoth-500/15 border-l-sky-400/60 bg-cosmos-700/40",
  warning: "border-amber-400/30 border-l-amber-400 bg-amber-400/[0.06]",
  critical: "border-rose-400/40 border-l-rose-400 bg-rose-500/[0.10]",
};

const BANNER_ICON_COLOR: Record<AnnouncementSeverity, string> = {
  info: "text-sky-300",
  warning: "text-amber-300",
  critical: "text-rose-300",
};

const BANNER_ICON: Record<AnnouncementSeverity, React.FC<IconProps>> = {
  info: InfoIcon,
  warning: AlertTriangleIcon,
  critical: AlertOctagonIcon,
};

export function AnnouncementBanner({
  announcement,
  dismissedIds,
}: {
  announcement: AnnouncementResponse;
  dismissedIds: number[];
}) {
  const t = useTranslations("Dashboard.Announcements");
  const [dismissed, setDismissed] = useState(false);

  if (dismissed) return null;

  const dismiss = () => {
    const value = serializeDismissedIds([...dismissedIds, announcement.id]);
    document.cookie = `${ANNOUNCEMENTS_DISMISSED_COOKIE}=${value}; path=/; max-age=${ANNOUNCEMENTS_COOKIE_MAX_AGE_S}; samesite=lax`;
    setDismissed(true);
  };

  const Icon = BANNER_ICON[announcement.severity];

  return (
    <div
      role="status"
      className={`mb-4 flex items-start gap-3 rounded-lg border border-l-2 px-4 py-3 ${BANNER_STYLE[announcement.severity]}`}
    >
      <Icon
        size={18}
        className={`mt-0.5 shrink-0 ${BANNER_ICON_COLOR[announcement.severity]}`}
      />
      <div className="min-w-0 flex-1">
        <span className="mr-2 font-mono text-[12px] tracking-wide text-slate-400 uppercase">
          {t(`kinds.${announcement.kind}`)}
        </span>
        <span className="text-[14px] text-slate-200">
          {announcement.message}
        </span>
        {announcement.linkUrl && (
          <Link
            href={announcement.linkUrl}
            className="ml-2 text-[13px] text-sky-300 underline underline-offset-2 hover:text-sky-200"
          >
            {t("readMore")}
          </Link>
        )}
      </div>
      <button
        type="button"
        onClick={dismiss}
        aria-label={t("dismiss")}
        className="shrink-0 rounded p-1 text-slate-400 transition-colors hover:bg-white/5 hover:text-slate-200"
      >
        <CloseIcon size={16} />
      </button>
    </div>
  );
}
