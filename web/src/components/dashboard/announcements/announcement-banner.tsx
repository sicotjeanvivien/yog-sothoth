"use client";

/**
 * The operator announcement banner. The server layout picks it from the cookie, so the first
 * paint is right; this island only dismisses. Its severity styling is its own, not the
 * signals' vocabulary: an announcement is editorial, not a detector's conclusion.
 */

import { useState } from "react";
import { useTranslations } from "next-intl";

import { Link } from "@/i18n/navigation";
import type {
  AnnouncementResponse,
  AnnouncementSeverityResponse,
} from "@/lib/api/schema/zod.gen";
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

// Shape and colour, never colour alone: each severity has its own icon.
const BANNER_RULE: Record<AnnouncementSeverityResponse, string> = {
  info: "border-l-dash-ink-3",
  warning: "border-l-dash-warn",
  critical: "border-l-dash-down",
};

const BANNER_ICON_COLOR: Record<AnnouncementSeverityResponse, string> = {
  info: "text-dash-ink-3",
  warning: "text-dash-warn",
  critical: "text-dash-down",
};

const BANNER_ICON: Record<AnnouncementSeverityResponse, React.FC<IconProps>> = {
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

  // ⚠️ Sticky, not in-flow: a reload restores the scroll, which would leave an in-flow banner
  // out of view. z-20 stays under the header, drawer and overlay.
  return (
    <div className="sticky top-14 z-20 border-b border-dash-rule bg-dash-surface font-dash-sans lg:top-0">
      <div
        role="status"
        className={`flex items-start gap-3 border-l-2 px-6 py-3 lg:px-10 ${BANNER_RULE[announcement.severity]}`}
      >
        <Icon
          size={18}
          className={`mt-0.5 shrink-0 ${BANNER_ICON_COLOR[announcement.severity]}`}
        />
        <div className="min-w-0 flex-1">
          <span className="mr-2 font-dash-mono text-[11px] tracking-[0.12em] text-dash-ink-3 uppercase">
            {t(`kinds.${announcement.kind}`)}
          </span>
          <span className="text-[14px] text-dash-ink">
            {announcement.message}
          </span>
          {announcement.linkUrl && (
            <Link
              href={announcement.linkUrl}
              className="ml-2 text-[13px] text-dash-accent underline underline-offset-2 hover:text-dash-ink"
            >
              {t("readMore")}
            </Link>
          )}
        </div>
        <button
          type="button"
          onClick={dismiss}
          aria-label={t("dismiss")}
          className="shrink-0 p-1 text-dash-ink-3 transition-colors hover:text-dash-ink"
        >
          <CloseIcon size={16} />
        </button>
      </div>
    </div>
  );
}
