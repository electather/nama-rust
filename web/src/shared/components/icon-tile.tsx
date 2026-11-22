import { cn } from "cn";
import type { LucideIcon } from "lucide-react";

const LOOKS = {
  plain: "bg-muted inset-ring inset-ring-border",
  // Something failed: DESIGN.md keeps Destructive the only status colour.
  destructive: "bg-destructive/15 text-destructive",
};

/** An icon on a neutral tile, or a red one for something that failed, at the start of a row of a list. */
export function IconTile({
  Icon,
  destructive = false,
}: {
  Icon: LucideIcon;
  destructive?: boolean;
}) {
  return (
    <span
      aria-hidden
      className={cn(
        "grid size-6 shrink-0 place-items-center rounded-md",
        LOOKS[destructive ? "destructive" : "plain"],
      )}
    >
      <Icon strokeWidth={1.5} className="size-3.5" />
    </span>
  );
}
