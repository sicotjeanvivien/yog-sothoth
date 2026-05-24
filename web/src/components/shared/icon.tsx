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
 