import { Link } from "@tanstack/react-router";
import { CircleCheck, Info, X } from "lucide-react";

import type { CardActions } from "@/features/home/lib/card-actions";
import {
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLinkItem,
} from "@/shared/ui/dropdown-menu";

/**
 * What a Continue Watching card's menu holds, each with its icon: its title's page, as Featured's
 * More info opens it; marking it watched, with the watched mark; removing it.
 */
export function CardMenuContent({ page, onMarkWatched, onRemove }: CardActions) {
  return (
    <DropdownMenuContent>
      <DropdownMenuLinkItem render={<Link {...page.link} />}>
        <Info aria-hidden className="size-4" />
        {page.label}
      </DropdownMenuLinkItem>
      <DropdownMenuItem onClick={onMarkWatched}>
        <CircleCheck aria-hidden className="size-4" />
        Mark as watched
      </DropdownMenuItem>
      <DropdownMenuItem onClick={onRemove}>
        <X aria-hidden className="size-4" />
        Remove from recently watched
      </DropdownMenuItem>
    </DropdownMenuContent>
  );
}
