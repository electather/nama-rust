import { DropdownMenuItem } from "@/shared/ui/dropdown-menu";

/** Delete, disabled, saying the Server can't write to `folder`, which holds the title. */
export function CantDeleteItem({ folder }: { folder: string }) {
  return (
    <DropdownMenuItem disabled>
      <span className="flex max-w-64 flex-col py-1.5">
        Delete
        <span className="text-xs wrap-break-word">
          The Server can’t write to {folder}. Make that folder writable, then scan the Library
          again.
        </span>
      </span>
    </DropdownMenuItem>
  );
}
