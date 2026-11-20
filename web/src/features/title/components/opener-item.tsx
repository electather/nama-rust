import type { ReactNode, RefObject } from "react";

import { useAfterMenuCloses } from "@/features/title/hooks/use-after-menu-closes";
import type { Opener } from "@/shared/lib/opener";
import { DropdownMenuItem } from "@/shared/ui/dropdown-menu";

/**
 * A menu item that opens the dialog `opener` holds, which lives outside the menu, once the menu
 * has closed, so the dialog hands focus back to the "…" when it closes.
 */
export function OpenerItem({
  opener,
  children,
}: {
  opener: RefObject<Opener | null>;
  children: ReactNode;
}) {
  const afterMenuCloses = useAfterMenuCloses();
  return (
    <DropdownMenuItem
      aria-haspopup="dialog"
      onClick={() => {
        afterMenuCloses(() => opener.current?.open());
      }}
    >
      {children}
    </DropdownMenuItem>
  );
}
