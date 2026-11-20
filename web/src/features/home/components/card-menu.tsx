import { CardMenuContent } from "@/features/home/components/card-menu-content";
import { CardMenuTrigger } from "@/features/home/components/card-menu-trigger";
import { DropdownMenu } from "@/shared/ui/dropdown-menu";

/**
 * A card's menu, over the top corner of its art, which stays dark, on the scrim (DESIGN.md, "The
 * Legible Over Art Rule").
 */
export function CardMenu({ title, onMarkWatched }: { title: string; onMarkWatched: () => void }) {
  return (
    <div className="dark absolute inset-e-2 top-2 rounded-lg bg-scrim">
      <DropdownMenu>
        <CardMenuTrigger title={title} />
        <CardMenuContent onMarkWatched={onMarkWatched} />
      </DropdownMenu>
    </div>
  );
}
