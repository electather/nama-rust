import { DropdownMenu } from "nama-ui/components/dropdown-menu";

import { CardMenuContent } from "@/features/home/components/card-menu-content";
import { CardMenuTrigger } from "@/features/home/components/card-menu-trigger";
import type { CardActions } from "@/features/home/lib/card-actions";

/**
 * A Continue Watching card's menu, at the end of its caption over the art, and the same items as
 * its right-click menu (`BackdropFrame`).
 */
export function CardMenu({ title, actions }: { title: string; actions: CardActions }) {
  return (
    <>
      <DropdownMenu>
        <CardMenuTrigger title={title} />
        <CardMenuContent {...actions} />
      </DropdownMenu>
      <CardMenuContent {...actions} context />
    </>
  );
}
