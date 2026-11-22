import { Ellipsis } from "lucide-react";

import type { Quality } from "@/gen/nama/v1/plugin_pb";
import { useModalContainer } from "@/shared/hooks/use-modal-container";
import { Button } from "@/shared/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/shared/ui/dropdown-menu";

/**
 * The "…" at the end of a Quality's row: editing it, making it the default unless it is, and
 * removing it. Edit is off without `onEdit`, while there are no profiles to pick from.
 */
export function QualityMenu({
  quality,
  onEdit,
  onDefault,
  onRemove,
}: {
  quality: Quality;
  onEdit: (() => void) | undefined;
  onDefault: () => void;
  onRemove: () => void;
}) {
  const [trigger, container] = useModalContainer();
  const more = (
    <Button ref={trigger} variant="ghost" size="icon-sm" aria-label={`More for ${quality.name}`} />
  );
  return (
    <DropdownMenu>
      <DropdownMenuTrigger render={more}>
        <Ellipsis aria-hidden />
      </DropdownMenuTrigger>
      <DropdownMenuContent container={container}>
        <DropdownMenuItem disabled={onEdit === undefined} onClick={onEdit}>
          Edit
        </DropdownMenuItem>
        {!quality.isDefault && (
          <DropdownMenuItem onClick={onDefault}>Make default</DropdownMenuItem>
        )}
        <DropdownMenuItem onClick={onRemove}>Remove</DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
