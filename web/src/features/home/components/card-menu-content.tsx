import { DropdownMenuContent, DropdownMenuItem } from "@/shared/ui/dropdown-menu";

/** What a card's menu holds: marking its Movie or Episode watched. */
export function CardMenuContent({ onMarkWatched }: { onMarkWatched: () => void }) {
  return (
    <DropdownMenuContent>
      <DropdownMenuItem onClick={onMarkWatched}>Mark as watched</DropdownMenuItem>
    </DropdownMenuContent>
  );
}
