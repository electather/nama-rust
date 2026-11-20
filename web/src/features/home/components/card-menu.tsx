import { CardMenuContent } from "@/features/home/components/card-menu-content";
import { CardMenuTrigger } from "@/features/home/components/card-menu-trigger";
import type { CardActions } from "@/features/home/lib/card-actions";
import { DropdownMenu } from "@/shared/ui/dropdown-menu";

/** A card's menu, at the end of its caption over the art, which stays dark (DESIGN.md, "The Legible Over Art Rule"). */
export function CardMenu({ title, actions }: { title: string; actions: CardActions }) {
  return (
    <div className="dark pointer-events-auto shrink-0 rounded-full">
      <DropdownMenu>
        <CardMenuTrigger title={title} />
        <CardMenuContent {...actions} />
      </DropdownMenu>
    </div>
  );
}
