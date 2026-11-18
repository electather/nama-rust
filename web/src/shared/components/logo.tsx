import { cn } from "cn";

import logo from "@/shared/components/logo.svg";

/**
 * The Nama mark, `size` px square. It's an image, not inline SVG, so its gradient ids can't clash
 * with the page's. Give it a `label` only when no written name stands beside it.
 */
export function Logo({
  size,
  label = "",
  className,
}: {
  size: number;
  label?: string;
  className?: string;
}) {
  return (
    <img
      src={logo}
      width={size}
      height={size}
      alt={label}
      draggable={false}
      className={cn("shrink-0", className)}
    />
  );
}
