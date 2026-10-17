/** The Yog-Scope mark, in the accent colour; decorative, its link or wordmark names it. */

import Image from "next/image";

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
    <Image
      src="/logo_yog_scope.svg"
      alt=""
      width={width}
      height={Math.round(width * HEIGHT_PER_WIDTH)}
      priority={priority}
      // ⚠️ The optimizer rejects SVG; served as is, it is cached once for every page.
      unoptimized
      className={`shrink-0 ${className}`}
    />
  );
}
