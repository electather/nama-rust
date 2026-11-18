import { DropdownMenuContent, DropdownMenuItem } from "@/shared/ui/dropdown-menu";

/** What a card's menu holds: hiding it from Continue Watching. */
export function HideMenuContent({ onHide }: { onHide: () => void }) {
  return (
    <DropdownMenuContent>
      <DropdownMenuItem onClick={onHide}>Hide from Continue Watching</DropdownMenuItem>
    </DropdownMenuContent>
  );
}
