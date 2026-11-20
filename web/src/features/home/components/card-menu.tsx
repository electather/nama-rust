import { CardMenuContent } from "@/features/home/components/card-menu-content";
import { CardMenuTrigger } from "@/features/home/components/card-menu-trigger";
import type { CardActions } from "@/features/home/lib/card-actions";
import { DropdownMenu } from "@/shared/ui/dropdown-menu";

/** A Continue Watching card's menu, at the end of its caption over the art. */
export function CardMenu({ title, actions }: { title: string; actions: CardActions }) {
  return (
    <DropdownMenu>
      <CardMenuTrigger title={title} />
      <CardMenuContent {...actions} />
    </DropdownMenu>
  );
}
