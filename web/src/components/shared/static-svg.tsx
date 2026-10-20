/** An SVG from `public/`, decorative: its row or link names it. */

import Image from "next/image";

export function StaticSvg({
  src,
  width,
  height,
  priority = false,
  className = "",
}: {
  src: string;
  width: number;
  height: number;
  priority?: boolean;
  className?: string;
}) {
  return (
    <Image
      src={src}
      alt=""
      width={width}
      height={height}
      priority={priority}
      loading={priority ? undefined : "eager"}
      // ⚠️ The optimizer rejects SVG; served as is, it is cached once for every page.
      unoptimized
      className={`shrink-0 ${className}`}
    />
  );
}
