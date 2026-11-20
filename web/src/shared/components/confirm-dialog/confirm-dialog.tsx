import { type Ref, useImperativeHandle, useRef, useState } from "react";

import { type Confirmation, ConfirmModal } from "@/shared/components/confirm-dialog/confirm-modal";
import type { Opener } from "@/shared/lib/opener";
import { Button } from "@/shared/ui/button";

/**
 * A button that asks in a dialog before doing something that can't be undone. The dialog stays
 * open, saying why, if the Server refuses, and closes once it's done. Without `trigger`, only `ref`
 * opens it, such as from a menu's item.
 */
export function ConfirmDialog({
  name,
  ref,
  trigger = true,
  ...confirmation
}: Confirmation & {
  /** The button's accessible name, which says what it acts on, such as "Revoke Laptop". */
  name: string;
  ref?: Ref<Opener> | undefined;
  trigger?: boolean | undefined;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [openings, setOpenings] = useState(0);
  const open = () => {
    setOpenings(openings + 1);
    dialog.current?.showModal();
  };
  useImperativeHandle(ref, () => ({ open }));
  return (
    <>
      {trigger && (
        <Button variant="ghost" aria-label={name} aria-haspopup="dialog" onClick={open}>
          {confirmation.action}
        </Button>
      )}
      <ConfirmModal ref={dialog} openings={openings} destructive {...confirmation}>
        {/* Only once opened, so what they fetch waits until someone asks. */}
        {openings > 0 && confirmation.children}
      </ConfirmModal>
    </>
  );
}
