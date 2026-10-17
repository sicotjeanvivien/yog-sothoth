import React from "react";

/**
 * The dashboard's icon set: a 24x24 viewBox, a 1.6px `currentColor` stroke and `shrink-0`.
 * `size` is in rendered pixels; the viewBox stays 24x24 so the line weight scales evenly.
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

/** One rung above `AlertTriangleIcon`: a distinct shape, so severity survives colour-blindness. */
export const AlertOctagonIcon: React.FC<IconProps> = ({
  size = 18,
  strokeWidth = 1.6,
  className,
}) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <path d="M8.2 3h7.6L21 8.2v7.6L15.8 21H8.2L3 15.8V8.2z" />
    <path d="M12 7.5v5" />
    <circle cx="12" cy="16" r="0.7" fill="currentColor" stroke="none" />
  </svg>
);

export const ArrowRightIcon: React.FC<IconProps> = ({
  size = 16,
  strokeWidth = 1.6,
  className,
}) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <path d="M5 12h14" />
    <path d="M12 5l7 7-7 7" />
  </svg>
);

export const StarIcon: React.FC<IconProps> = ({
  size = 16,
  strokeWidth = 1.6,
  className,
}) => (
  <svg {...base(size, strokeWidth)} className={className}>
    <path d="M12 2.7l2.85 5.85 6.45.9-4.65 4.5 1.05 6.45L12 17.4l-5.7 3 1.05-6.45-4.65-4.5 6.45-.9z" />
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
