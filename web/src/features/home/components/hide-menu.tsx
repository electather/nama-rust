import { HideMenuContent } from "@/features/home/components/hide-menu-content";
import { HideMenuTrigger } from "@/features/home/components/hide-menu-trigger";
import { DropdownMenu } from "@/shared/ui/dropdown-menu";

/**
 * A card's menu, over the top corner of its art, which stays dark, on the scrim (DESIGN.md, "The
 * Legible Over Art Rule").
 */
export function HideMenu({ title, onHide }: { title: string; onHide: () => void }) {
  return (
    <div className="dark absolute inset-e-2 top-2 rounded-lg bg-scrim">
      <DropdownMenu>
        <HideMenuTrigger title={title} />
        <HideMenuContent onHide={onHide} />
      </DropdownMenu>
    </div>
  );
}
