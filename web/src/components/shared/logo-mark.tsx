/** The Yog-Scope mark, in the accent colour; decorative, its link or wordmark names it. */

import { StaticSvg } from "./static-svg";

// The file's viewBox, 873 × 975.
const HEIGHT_PER_WIDTH = 975 / 873;

export function LogoMark({
  width,
  priority = false,
  className = "",
}: {
  width: number;
  priority?: boolean;
  className?: string;
}) {
  return (
    <StaticSvg
      src="/logo_yog_scope.svg"
      width={width}
      height={Math.round(width * HEIGHT_PER_WIDTH)}
      priority={priority}
      className={className}
    />
  );
}
