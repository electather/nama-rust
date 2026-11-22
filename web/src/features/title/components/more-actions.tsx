import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuTrigger,
} from "nama-ui/components/dropdown-menu";
import { type ReactNode, useRef } from "react";

import { MoreActionsButton } from "@/features/title/components/more-actions-button";
import { AfterMenuCloses } from "@/features/title/hooks/use-after-menu-closes";

/** The "…" named `label`, `quiet` on a card, and the menu of `children`, its items, it opens. */
export function MoreActions({
  label,
  quiet,
  children,
}: {
  label: string;
  quiet?: boolean | undefined;
  children: ReactNode;
}) {
  const after = useRef<() => void>(undefined);
  const trigger = useRef<HTMLButtonElement>(null);
  const button = <MoreActionsButton ref={trigger} label={label} quiet={quiet} />;
  return (
    <AfterMenuCloses
      value={(action) => {
        after.current = action;
      }}
    >
      <DropdownMenu
        onOpenChangeComplete={(open) => {
          // Focus is on the "…" before the dialog opens, which hands it back there when it closes.
          // Opened by a click, it would hand it back to the item, which is gone, so to the page.
          if (!open && after.current) {
            trigger.current?.focus();
            after.current();
            after.current = undefined;
          }
        }}
      >
        <DropdownMenuTrigger render={button} />
        <DropdownMenuContent>{children}</DropdownMenuContent>
      </DropdownMenu>
    </AfterMenuCloses>
  );
}
