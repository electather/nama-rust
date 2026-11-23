import { cn } from "cn";
import {
  Film,
  House,
  Keyboard,
  type LucideIcon,
  RefreshCw,
  Settings,
  Tv,
  UserRound,
} from "lucide-react";

import type { IconName } from "@/features/action-bar/lib/rows";

const icons = {
  movie: Film,
  show: Tv,
  person: UserRound,
  home: House,
  settings: Settings,
  keyboard: Keyboard,
  retry: RefreshCw,
} satisfies Record<IconName, LucideIcon>;

/**
 * The icon a row leads with, in a 32px box. The box has a rim, unless it's `bare`: standing in for
 * a title's missing poster, it's drawn as the poster would be.
 */
export function RowIcon({ icon, bare }: { icon: IconName; bare: boolean }) {
  const Icon = icons[icon];
  return (
    <span
      aria-hidden
      className={cn(
        "mx-1 grid size-8 shrink-0 place-items-center rounded-sm bg-muted text-foreground",
        !bare && "border",
      )}
    >
      <Icon strokeWidth={1.5} className="size-3.5" />
    </span>
  );
}
