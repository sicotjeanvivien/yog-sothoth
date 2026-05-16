import React from "react";

/**
 * Icon set for the Yog-Sothoth sidebar.
 * All icons are 1.5px stroked, currentColor based, 24x24 viewBox.
 */

export type IconProps = {
  size?: number;
  className?: string;
  strokeWidth?: number;
};

const base = (size: number, strokeWidth: number) => ({
  width: size,
  height: size,
  viewBox: "0 0 24 24",
  fill: "none",
  stroke: "currentColor",
  strokeWidth,
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
});

export const HomeIcon: React.FC<IconProps> = ({ size = 18, strokeWidth = 1.6, className }) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <path d="M3 10.5 12 3l9 7.5V20a1 1 0 0 1-1 1h-5v-6h-6v6H4a1 1 0 0 1-1-1z" />
  </svg>
);

export const PoolsIcon: React.FC<IconProps> = ({ size = 18, strokeWidth = 1.6, className }) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <circle cx="8" cy="9" r="3.2" />
    <circle cx="16" cy="9" r="3.2" />
    <circle cx="8" cy="17" r="3.2" />
    <circle cx="16" cy="17" r="3.2" />
  </svg>
);

export const LiquidityIcon: React.FC<IconProps> = ({ size = 18, strokeWidth = 1.6, className }) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <path d="M12 3.5c3.2 4 6 7.1 6 10.5a6 6 0 1 1-12 0c0-3.4 2.8-6.5 6-10.5z" />
  </svg>
);

export const FlowsIcon: React.FC<IconProps> = ({ size = 18, strokeWidth = 1.6, className }) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <path d="M4 8h13" />
    <path d="m14 5 3 3-3 3" />
    <path d="M20 16H7" />
    <path d="m10 19-3-3 3-3" />
  </svg>
);

export const SignalsIcon: React.FC<IconProps> = ({ size = 18, strokeWidth = 1.6, className }) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <path d="M4.5 12.5a10 10 0 0 1 15 0" />
    <path d="M7.5 15.5a6 6 0 0 1 9 0" />
    <circle cx="12" cy="18.5" r="1.1" fill="currentColor" stroke="none" />
  </svg>
);

export const AlertsIcon: React.FC<IconProps> = ({ size = 18, strokeWidth = 1.6, className }) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <path d="M6 16V11a6 6 0 1 1 12 0v5l1.5 2.2a.5.5 0 0 1-.4.8H4.9a.5.5 0 0 1-.4-.8z" />
    <path d="M10.5 21.5a2 2 0 0 0 3 0" />
  </svg>
);

export const WatchlistIcon: React.FC<IconProps> = ({ size = 18, strokeWidth = 1.6, className }) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <path d="m12 3.7 2.65 5.37 5.93.86-4.29 4.18 1.01 5.9L12 17.22 6.7 20l1.01-5.9-4.29-4.18 5.93-.86z" />
  </svg>
);

export const SettingsIcon: React.FC<IconProps> = ({ size = 18, strokeWidth = 1.6, className }) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <circle cx="12" cy="12" r="2.6" />
    <path d="M19.4 14.5a1.7 1.7 0 0 0 .34 1.87l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.7 1.7 0 0 0-1.87-.34 1.7 1.7 0 0 0-1.03 1.56V20a2 2 0 0 1-4 0v-.08a1.7 1.7 0 0 0-1.11-1.56 1.7 1.7 0 0 0-1.87.34l-.06.06A2 2 0 1 1 4.14 15.93l.06-.06a1.7 1.7 0 0 0 .34-1.87 1.7 1.7 0 0 0-1.56-1.03H3a2 2 0 0 1 0-4h.08A1.7 1.7 0 0 0 4.64 7.9a1.7 1.7 0 0 0-.34-1.87l-.06-.06A2 2 0 1 1 7.07 3.14l.06.06a1.7 1.7 0 0 0 1.87.34H9a1.7 1.7 0 0 0 1.03-1.56V2a2 2 0 0 1 4 0v.08a1.7 1.7 0 0 0 1.03 1.56 1.7 1.7 0 0 0 1.87-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.7 1.7 0 0 0-.34 1.87V8a1.7 1.7 0 0 0 1.56 1.03H21a2 2 0 0 1 0 4h-.08a1.7 1.7 0 0 0-1.56 1.03z" />
  </svg>
);

export const ChevronDownIcon: React.FC<IconProps> = ({ size = 14, strokeWidth = 1.8, className }) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <path d="m6 9 6 6 6-6" />
  </svg>
);

/**
 * Yog-Sothoth brand mark: 4-pointed star/diamond with an inner eye + vertical slit pupil.
 * Drawn with a layered radial glow so it reads on dark UIs.
 */
export const YogSothothMark: React.FC<{ size?: number }> = ({ size = 72 }) => {
  const id = React.useId();
  return (
    <svg width={size} height={size} viewBox="0 0 100 100" fill="none" aria-hidden>
      <defs>
        <radialGradient id={`${id}-glow`} cx="50%" cy="50%" r="50%">
          <stop offset="0%" stopColor="#A78BFA" stopOpacity="0.55" />
          <stop offset="60%" stopColor="#7C3AED" stopOpacity="0.08" />
          <stop offset="100%" stopColor="#7C3AED" stopOpacity="0" />
        </radialGradient>
        <linearGradient id={`${id}-stroke`} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0%" stopColor="#C4B5FD" />
          <stop offset="50%" stopColor="#8B5CF6" />
          <stop offset="100%" stopColor="#6D28D9" />
        </linearGradient>
        <linearGradient id={`${id}-iris`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor="#C4B5FD" />
          <stop offset="100%" stopColor="#7C3AED" />
        </linearGradient>
      </defs>
      <circle cx="50" cy="50" r="46" fill={`url(#${id}-glow)`} />
      {/* 4-pointed star diamond outline */}
      <path
        d="M50 6 L62 38 L94 50 L62 62 L50 94 L38 62 L6 50 L38 38 Z"
        stroke={`url(#${id}-stroke)`}
        strokeWidth="1.5"
        fill="rgba(124, 58, 237, 0.08)"
      />
      <path
        d="M50 18 L58 42 L82 50 L58 58 L50 82 L42 58 L18 50 L42 42 Z"
        stroke="rgba(196, 181, 253, 0.35)"
        strokeWidth="0.8"
        fill="none"
      />
      {/* Eye almond shape */}
      <path
        d="M28 50 Q50 32 72 50 Q50 68 28 50 Z"
        fill="#0A0B12"
        stroke={`url(#${id}-stroke)`}
        strokeWidth="1.2"
      />
      {/* Iris */}
      <ellipse cx="50" cy="50" rx="9" ry="11" fill={`url(#${id}-iris)`} />
      {/* Vertical slit pupil */}
      <ellipse cx="50" cy="50" rx="1.6" ry="8" fill="#0A0B12" />
      {/* Highlight */}
      <circle cx="46.5" cy="46" r="1.3" fill="#F5F3FF" opacity="0.9" />
    </svg>
  );
};

/**
 * Stylised Solana-style network glyph (three angled bars in a rounded square).
 * Generic chain-mark — not an exact reproduction of any brand asset.
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
      <rect x="0.5" y="0.5" width="31" height="31" rx="8" fill={`url(#${id}-bg)`} stroke="rgba(153, 69, 255, 0.25)" />
      <path d="M10 11 L23 11 L20.5 13.5 L7.5 13.5 Z" fill={`url(#${id}-bar)`} />
      <path d="M7.5 15.25 L20.5 15.25 L23 17.75 L10 17.75 Z" fill={`url(#${id}-bar)`} opacity="0.85" />
      <path d="M10 19.5 L23 19.5 L20.5 22 L7.5 22 Z" fill={`url(#${id}-bar)`} opacity="0.7" />
    </svg>
  );
};
