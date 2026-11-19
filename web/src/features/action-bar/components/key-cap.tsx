import { cn } from "cn";
import { ChevronDown, ChevronUp, CornerDownLeft, type LucideIcon } from "lucide-react";

// Keys drawn rather than spelled, each with the name a screen reader says.
const drawn: Record<string, [LucideIcon, string] | undefined> = {
  up: [ChevronUp, "Up"],
  down: [ChevronDown, "Down"],
  enter: [CornerDownLeft, "Enter"],
};

/**
 * One key of the keyboard, as a chip: `name` spelled out, or drawn for "up", "down" and "enter".
 * `rim` adds a hairline, for a key on the popover's own fill or a selected row's.
 */
export function KeyCap({ name, rim = false }: { name: string; rim?: boolean }) {
  const [Icon, label] = drawn[name] ?? [];
  return (
    <kbd
      className={cn(
        "inline-flex h-5 min-w-5 shrink-0 items-center justify-center rounded-sm bg-muted px-1 font-sans text-xs font-medium text-foreground select-none",
        rim && "border",
      )}
    >
      {Icon ? <Icon aria-label={label} strokeWidth={1.5} className="size-3" /> : name}
    </kbd>
  );
}
