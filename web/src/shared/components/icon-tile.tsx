import type { LucideIcon } from "lucide-react";

/** An icon on a neutral tile, at the start of a row of a list. */
export function IconTile({ Icon }: { Icon: LucideIcon }) {
  return (
    <span
      aria-hidden
      className="grid size-8 shrink-0 place-items-center rounded-md bg-muted inset-ring inset-ring-border"
    >
      <Icon strokeWidth={1.5} className="size-4" />
    </span>
  );
}
