import { Pencil } from "lucide-react";

import { useModalContainer } from "@/shared/hooks/use-modal-container";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/shared/ui/dropdown-menu";

/**
 * The pencil at the corner of a Plugin's logo, and what it opens: uploading an image to show in the
 * logo's place and, with `onRemove`, taking that image away. It shows while the pointer is over the
 * logo (its `group/logo`), while it has focus or its menu is open, and always on a touch screen,
 * which has no pointer to hover with.
 */
export function LogoMenu({
  pending,
  onUpload,
  onRemove,
}: {
  pending: boolean;
  onUpload: () => void;
  onRemove?: (() => void) | undefined;
}) {
  const [trigger, container] = useModalContainer();
  const pencil = (
    <button
      ref={trigger}
      type="button"
      aria-label="Change the logo"
      title="Change the logo"
      disabled={pending}
      // Its hit area reaches 44px past its 28px circle, for a fingertip.
      className="absolute -inset-e-2 -top-2 grid size-7 cursor-pointer place-items-center rounded-full border bg-popover text-popover-foreground opacity-0 shadow-md transition-opacity outline-none group-hover/logo:opacity-100 after:absolute after:-inset-2 hover:bg-accent focus-visible:opacity-100 focus-visible:ring-3 focus-visible:ring-ring/50 disabled:pointer-events-none disabled:text-muted-foreground data-popup-open:opacity-100 motion-reduce:transition-none pointer-coarse:opacity-100"
    />
  );
  return (
    <DropdownMenu>
      <DropdownMenuTrigger render={pencil}>
        <Pencil aria-hidden className="size-3.5" />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" container={container}>
        <DropdownMenuItem onClick={onUpload}>Upload an image</DropdownMenuItem>
        {onRemove !== undefined && (
          <DropdownMenuItem onClick={onRemove}>Remove custom image</DropdownMenuItem>
        )}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
