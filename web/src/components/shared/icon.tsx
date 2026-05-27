import React from "react";

/**
 * Icon set for the Yog-Sothoth dashboard.
 *
 * All icons share one geometry contract:
 *   - 24x24 viewBox,
 *   - 1.6px stroke by default, `currentColor` based (they shade with
 *     the surrounding text colour),
 *   - `shrink-0` so they never get squeezed by a flex parent.
 *
 * Sizes are expressed in rendered pixels via the `size` prop; the
 * viewBox stays 24x24 regardless so the line weight scales evenly.
 */

export type IconProps = {
  size?: number;
  className?: string;
  strokeWidth?: number;
};

/**
 * Shared SVG attributes. The viewBox is 24x24 — every path below is
 * authored against that coordinate space.
 */
const base = (size: number, strokeWidth: number) => ({
  width: size,
  height: size,
  viewBox: "0 0 24 24",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: strokeWidth,
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
  className: "shrink-0",
  "aria-hidden": true,
});

export const OverviewIcon: React.FC<IconProps> = ({
  size = 18,
  strokeWidth = 1.6,
  className,
}) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <path d="M3 10.5 12 3l9 7.5V20a1 1 0 0 1-1 1h-5v-6h-6v6H4a1 1 0 0 1-1-1z" />
  </svg>
);

/**
 * Pools — stacked layers. Matches the validated sidebar prototype:
 * a top rhombus over two parallel "sheets". Authored on the 24x24
 * grid (top vertex at y=3, base sheet near y=20).
 */
export const PoolsIcon: React.FC<IconProps> = ({
  size = 18,
  strokeWidth = 1.6,
  className,
}) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <path d="M12 3 21 7.5 12 12 3 7.5Z" />
    <path d="M3 12 12 16.5 21 12" />
    <path d="M3 16.5 12 21 21 16.5" />
  </svg>
);

export const LiquidityIcon: React.FC<IconProps> = ({
  size = 18,
  strokeWidth = 1.6,
  className,
}) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <path d="M12 3.5c3.2 4 6 7.1 6 10.5a6 6 0 1 1-12 0c0-3.4 2.8-6.5 6-10.5z" />
  </svg>
);

export const FlowsIcon: React.FC<IconProps> = ({
  size = 18,
  strokeWidth = 1.6,
  className,
}) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <path d="M4 8h13" />
    <path d="m14 5 3 3-3 3" />
    <path d="M20 16H7" />
    <path d="m10 19-3-3 3-3" />
  </svg>
);

export const SignalsIcon: React.FC<IconProps> = ({
  size = 18,
  strokeWidth = 1.6,
  className,
}) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <path d="M4.5 12.5a10 10 0 0 1 15 0" />
    <path d="M7.5 15.5a6 6 0 0 1 9 0" />
    <circle cx="12" cy="18.5" r="1.1" fill="currentColor" stroke="none" />
  </svg>
);

export const AlertsIcon: React.FC<IconProps> = ({
  size = 18,
  strokeWidth = 1.6,
  className,
}) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <path d="M6 16V11a6 6 0 1 1 12 0v5l1.5 2.2a.5.5 0 0 1-.4.8H4.9a.5.5 0 0 1-.4-.8z" />
    <path d="M10.5 21.5a2 2 0 0 0 3 0" />
  </svg>
);

export const WatchlistIcon: React.FC<IconProps> = ({
  size = 18,
  strokeWidth = 1.6,
  className,
}) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <path d="m12 3.7 2.65 5.37 5.93.86-4.29 4.18 1.01 5.9L12 17.22 6.7 20l1.01-5.9-4.29-4.18 5.93-.86z" />
  </svg>
);

export const SettingsIcon: React.FC<IconProps> = ({
  size = 18,
  strokeWidth = 1.6,
  className,
}) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <circle cx="12" cy="12" r="2.6" />
    <path d="M19.4 14.5a1.7 1.7 0 0 0 .34 1.87l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.7 1.7 0 0 0-1.87-.34 1.7 1.7 0 0 0-1.03 1.56V20a2 2 0 0 1-4 0v-.08a1.7 1.7 0 0 0-1.11-1.56 1.7 1.7 0 0 0-1.87.34l-.06.06A2 2 0 1 1 4.14 15.93l.06-.06a1.7 1.7 0 0 0 .34-1.87 1.7 1.7 0 0 0-1.56-1.03H3a2 2 0 0 1 0-4h.08A1.7 1.7 0 0 0 4.64 7.9a1.7 1.7 0 0 0-.34-1.87l-.06-.06A2 2 0 1 1 7.07 3.14l.06.06a1.7 1.7 0 0 0 1.87.34H9a1.7 1.7 0 0 0 1.03-1.56V2a2 2 0 0 1 4 0v.08a1.7 1.7 0 0 0 1.03 1.56 1.7 1.7 0 0 0 1.87-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.7 1.7 0 0 0-.34 1.87V8a1.7 1.7 0 0 0 1.56 1.03H21a2 2 0 0 1 0 4h-.08a1.7 1.7 0 0 0-1.56 1.03z" />
  </svg>
);

export const ChevronDownIcon: React.FC<IconProps> = ({
  size = 14,
  strokeWidth = 1.8,
  className,
}) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <path d="m6 9 6 6 6-6" />
  </svg>
);

/**
 * Stylised Solana-style network glyph (three angled bars in a rounded
 * square). Generic chain-mark — not an exact reproduction of any
 * brand asset.
 */
export const SolanaGlyph: React.FC<{ size?: number }> = ({ size = 28 }) => {
  const id = React.useId();
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" fill="none" aria-hidden>
      <defs>
        <linearGradient id={`${id}-bg`} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0%" stopColor="#1A1B2E" />
          <stop offset="100%" stopColor="#0F1020" />
        </linearGradient>
        <linearGradient id={`${id}-bar`} x1="0" y1="0" x2="1" y2="0">
          <stop offset="0%" stopColor="#9945FF" />
          <stop offset="100%" stopColor="#14F195" />
        </linearGradient>
      </defs>
      <rect
        x="0.5"
        y="0.5"
        width="31"
        height="31"
        rx="8"
        fill={`url(#${id}-bg)`}
        stroke="rgba(153, 69, 255, 0.25)"
      />
      <path d="M10 11 L23 11 L20.5 13.5 L7.5 13.5 Z" fill={`url(#${id}-bar)`} />
      <path
        d="M7.5 15.25 L20.5 15.25 L23 17.75 L10 17.75 Z"
        fill={`url(#${id}-bar)`}
        opacity="0.85"
      />
      <path
        d="M10 19.5 L23 19.5 L20.5 22 L7.5 22 Z"
        fill={`url(#${id}-bar)`}
        opacity="0.7"
      />
    </svg>
  );
};

/**
 * Hamburger — three stacked lines. Used by the mobile drawer trigger
 * in the dashboard shell. Authored on the 24x24 grid like the rest of
 * the set.
 */
export const HamburgerIcon: React.FC<IconProps> = ({
  size = 20,
  strokeWidth = 1.6,
  className,
}) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <path d="M4 7h16M4 12h16M4 17h16" />
  </svg>
);

export const CloseIcon: React.FC<IconProps> = ({
  size = 20,
  strokeWidth = 1.6,
  className,
}) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <path d="M6 6l12 12M18 6L6 18" />
  </svg>
);

export const ArrowRightIcon: React.FC<IconProps> = ({
  size = 14,
  strokeWidth = 1.6,
  className,
}) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <path d="M4 10h12M11 5l5 5-5 5" />
  </svg>
);

/**
 * Eye — used by the "Observe" feature pillar. A simple almond outline
 * with a round pupil. Echoes the brand sigil without reproducing it.
 */
export const EyeIcon: React.FC<IconProps> = ({
  size = 18,
  strokeWidth = 1.6,
  className,
}) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <path d="M2 12c3-5.5 7-8 10-8s7 2.5 10 8c-3 5.5-7 8-10 8s-7-2.5-10-8z" />
    <circle cx="12" cy="12" r="3.2" />
  </svg>
);

/**
 * Pulse — used by the "Analyze" feature pillar. A heartbeat / signal
 * trace: a flat baseline broken by a spike.
 */
export const PulseIcon: React.FC<IconProps> = ({
  size = 18,
  strokeWidth = 1.6,
  className,
}) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <path d="M2 12h5l3-7 4 14 3-7h5" />
  </svg>
);

/** X (formerly Twitter) glyph. */
export const XIcon: React.FC<IconProps> = ({
  size = 16,
  strokeWidth = 1.6,
  className,
}) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <path d="M18.244 2.25h3.308l-7.227 8.26 8.502 11.24H16.17l-5.214-6.817L4.99 21.75H1.68l7.73-8.835L1.254 2.25H8.08l4.713 6.231zm-1.161 17.52h1.833L7.084 4.126H5.117z" />
  </svg>
);

/** GitHub mark. */
export const GithubIcon: React.FC<IconProps> = ({
  size = 16,
  strokeWidth = 1.6,
  className,
}) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <path d="M12 .297c-6.63 0-12 5.373-12 12 0 5.303 3.438 9.8 8.205 11.385.6.113.82-.258.82-.577 0-.285-.01-1.04-.015-2.04-3.338.724-4.042-1.61-4.042-1.61-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23A11.5 11.5 0 0 1 12 5.803c1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.91 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.371.823 1.102.823 2.222 0 1.606-.014 2.898-.014 3.293 0 .322.216.694.825.576C20.565 22.092 24 17.595 24 12.297c0-6.627-5.373-12-12-12" />
  </svg>
);

/**
 * Open source — an open padlock. Symbolises unlocked / freely
 * accessible. Used to mark the "How it's available" section on the
 * About page, in the same visual family as `EyeIcon` and
 * `PulseIcon`. The shackle is drawn open on the right side; the
 * body is a rounded square.
 */
export const OpenSourceIcon: React.FC<IconProps> = ({
  size = 18,
  strokeWidth = 1.6,
  className,
}) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <rect x="4" y="11" width="14" height="9" rx="1.5" />
    <path d="M7 11V8a4 4 0 0 1 7.5-2" />
    <circle cx="11" cy="15.5" r="1.2" fill="currentColor" stroke="none" />
  </svg>
);

/**
 * Users — two overlapping silhouettes. Used to mark the "Who is
 * behind it" section on the About page. A larger figure on the left
 * and a smaller one behind on the right; both share the simple
 * head-circle plus rounded-shoulder torso pattern of generic user
 * glyphs. Authored on the 24x24 grid.
 */
export const UsersIcon: React.FC<IconProps> = ({
  size = 18,
  strokeWidth = 1.6,
  className,
}) => (
  <svg {...base(size, strokeWidth)} className={className}>
    {/* Front figure */}
    <circle cx="9" cy="8" r="3" />
    <path d="M3 20v-1a5 5 0 0 1 5-5h2a5 5 0 0 1 5 5v1" />
    {/* Back figure — slightly offset, smaller arc */}
    <path d="M16 11a3 3 0 1 0-2-5.2" />
    <path d="M16.5 14h.5a4 4 0 0 1 4 4v1" />
  </svg>
);

/**
 * Info — outlined circle with a vertical accent. Used as a "TL;DR"
 * marker on policy pages.
 */
export const InfoIcon: React.FC<IconProps> = ({
  size = 18,
  strokeWidth = 1.6,
  className,
}) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <circle cx="12" cy="12" r="9" />
    <path d="M12 11v5" />
    <circle cx="12" cy="8" r="0.6" fill="currentColor" stroke="none" />
  </svg>
);

/**
 * User card — a single silhouette inside a framed card. Used to
 * identify the data controller / responsible party. Distinct from
 * `UsersIcon` (which depicts multiple people) so the same page can
 * use both without semantic overlap.
 */
export const UserCardIcon: React.FC<IconProps> = ({
  size = 18,
  strokeWidth = 1.6,
  className,
}) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <rect x="3" y="5" width="18" height="14" rx="1.5" />
    <circle cx="9" cy="11" r="2" />
    <path d="M6 16.5a3.5 3.5 0 0 1 6 0" />
    <path d="M14 10h4" />
    <path d="M14 13h3" />
  </svg>
);

/**
 * Cookie — disc with three "chip" dots. Used to mark cookie-related
 * sections on policy pages.
 */
export const CookieIcon: React.FC<IconProps> = ({
  size = 18,
  strokeWidth = 1.6,
  className,
}) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <path d="M12 3a9 9 0 1 0 9 9 4 4 0 0 1-4-4 4 4 0 0 1-5-5z" />
    <circle cx="9" cy="11" r="0.8" fill="currentColor" stroke="none" />
    <circle cx="14" cy="14" r="0.8" fill="currentColor" stroke="none" />
    <circle cx="10" cy="15.5" r="0.6" fill="currentColor" stroke="none" />
  </svg>
);

/**
 * Shield — a closed-loop shield outline. Used to mark
 * rights / protection sections on policy pages. Distinct from
 * `OpenSourceIcon` (an opened padlock) since this one signals
 * protection rather than openness.
 */
export const ShieldIcon: React.FC<IconProps> = ({
  size = 18,
  strokeWidth = 1.6,
  className,
}) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <path d="M12 3 4 6v6c0 4.5 3.4 7.8 8 9 4.6-1.2 8-4.5 8-9V6z" />
  </svg>
);

/**
 * Refresh — circular arrow. Used to mark "changes to this policy"
 * sections — the policy is a living document and may be updated.
 */
export const RefreshIcon: React.FC<IconProps> = ({
  size = 18,
  strokeWidth = 1.6,
  className,
}) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <path d="M21 12a9 9 0 1 1-3-6.7" />
    <path d="M21 4v5h-5" />
  </svg>
);

/**
 * Server — two stacked horizontal racks with a status LED on each.
 * Used to mark the hosting provider section on legal pages.
 */
export const ServerIcon: React.FC<IconProps> = ({
  size = 18,
  strokeWidth = 1.6,
  className,
}) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <rect x="3" y="5" width="18" height="6" rx="1.5" />
    <rect x="3" y="13" width="18" height="6" rx="1.5" />
    <circle cx="7" cy="8" r="0.7" fill="currentColor" stroke="none" />
    <circle cx="7" cy="16" r="0.7" fill="currentColor" stroke="none" />
    <path d="M11 8h6" />
    <path d="M11 16h6" />
  </svg>
);

/**
 * Edit / pen-on-paper — used to mark the publishing director
 * section on legal pages. A simple sheet with a pen overlay.
 */
export const EditIcon: React.FC<IconProps> = ({
  size = 18,
  strokeWidth = 1.6,
  className,
}) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <path d="M4 6a2 2 0 0 1 2-2h7l5 5v9a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2z" />
    <path d="M13 4v5h5" />
    <path d="m10.5 17 5-5 1.5 1.5-5 5H10.5z" />
  </svg>
);

/**
 * Alert triangle — equilateral triangle with an exclamation mark.
 * Used to mark warning sections on policy pages.
 */
export const AlertTriangleIcon: React.FC<IconProps> = ({
  size = 18,
  strokeWidth = 1.6,
  className,
}) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <path d="M12 3.5 21 19H3z" />
    <path d="M12 10v4.5" />
    <circle cx="12" cy="17" r="0.7" fill="currentColor" stroke="none" />
  </svg>
);

/**
 * LinkedIn mark — used for share buttons. Authored on the 24x24
 * grid, fill rather than stroke (the geometry is too dense for the
 * 1.6px stroke contract).
 */
export const LinkedinIcon: React.FC<IconProps> = ({
  size = 16,
  strokeWidth = 1.6,
  className,
}) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <path
      d="M20.45 20.45h-3.55v-5.57c0-1.33-.03-3.04-1.85-3.04-1.85 0-2.13 1.45-2.13 2.94v5.67H9.37V9h3.41v1.56h.05c.47-.9 1.63-1.85 3.36-1.85 3.6 0 4.27 2.37 4.27 5.45v6.29zM5.34 7.43A2.06 2.06 0 1 1 5.34 3.3a2.06 2.06 0 0 1 0 4.13zM7.12 20.45H3.56V9h3.56v11.45z"
      fill="currentColor"
      stroke="none"
    />
  </svg>
);

/**
 * Mail — outlined envelope. Used to mark email contact links.
 */
export const MailIcon: React.FC<IconProps> = ({
  size = 18,
  strokeWidth = 1.6,
  className,
}) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <rect x="3" y="5" width="18" height="14" rx="1.5" />
    <path d="m3.5 6.5 8.5 6 8.5-6" />
  </svg>
);

/**
 * Copy — outlined "two squares" copy affordance. 1.6px stroke.
 */
export const CopyIcon: React.FC<IconProps> = ({
  size = 16,
  strokeWidth = 1.6,
  className,
}) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <rect x="9" y="9" width="11" height="11" rx="2" />
    <path d="M5 15H4a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1h10a1 1 0 0 1 1 1v1" />
  </svg>
);

/**
 * Check — outlined tick mark. Used as the copy confirmation state.
 */
export const CheckIcon: React.FC<IconProps> = ({
  size = 16,
  strokeWidth = 1.8,
  className,
}) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <path d="M4 12l5 5L20 6" />
  </svg>
);

/**
 * External link — outlined "arrow out of box". Marks links that
 * leave the site.
 */
export const ExternalLinkIcon: React.FC<IconProps> = ({
  size = 14,
  strokeWidth = 1.6,
  className,
}) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <path d="M14 5h5v5" />
    <path d="M19 5l-9 9" />
    <path d="M19 13v5a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1h5" />
  </svg>
);

/**
 * ArrowLeft — used for "back" navigation links.
 */
export const ArrowLeftIcon: React.FC<IconProps> = ({
  size = 16,
  strokeWidth = 1.6,
  className,
}) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <path d="M19 12H5" />
    <path d="M12 5l-7 7 7 7" />
  </svg>
);

export const ChevronDoubleLeftIcon: React.FC<IconProps> = ({
  size = 24,
  strokeWidth = 2,
  className,
}) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <polyline points="11 17 6 12 11 7" />
    <polyline points="18 17 13 12 18 7" />
  </svg>
);

export const ChevronDoubleRightIcon: React.FC<IconProps> = ({
  size = 24,
  strokeWidth = 2,
  className,
}) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <polyline points="13 17 18 12 13 7" />
    <polyline points="6 17 11 12 6 7" />
  </svg>
);

export const ChevronLeftIcon: React.FC<IconProps> = ({
  size = 24,
  strokeWidth = 2,
  className,
}) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <polyline points="15 18 9 12 15 6" />
  </svg>
);

export const ChevronRightIcon: React.FC<IconProps> = ({
  size = 24,
  strokeWidth = 2,
  className,
}) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <polyline points="9 18 15 12 9 6" />
  </svg>
);
