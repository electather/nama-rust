import { ContextMenuContent } from "nama-ui/components/context-menu";
import type { ReactNode } from "react";

import { MoreActions } from "@/features/title/components/more-actions";

/**
 * A card's "…", named `label`, and its right-click menu (`BackdropFrame`), both of `children`, its
 * items. An item that opens a dialog opens it at once from the right-click menu, which has no "…"
 * to hand focus back to.
 */
export function CardMoreActions({ label, children }: { label: string; children: ReactNode }) {
  return (
    <>
      <MoreActions label={label} quiet>
        {children}
      </MoreActions>
      <ContextMenuContent>{children}</ContextMenuContent>
    </>
  );
}
