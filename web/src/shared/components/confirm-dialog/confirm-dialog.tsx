import { useRef, useState } from "react";

import { type Confirmation, ConfirmModal } from "@/shared/components/confirm-dialog/confirm-modal";
import { Button } from "@/shared/ui/button";

/**
 * A button that asks in a dialog before doing something that can't be undone. The dialog stays
 * open, saying why, if the Server refuses, and closes once it's done.
 */
export function ConfirmDialog({
  name,
  ...confirmation
}: Confirmation & {
  /** The button's accessible name, which says what it acts on, such as "Revoke Laptop". */
  name: string;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [openings, setOpenings] = useState(0);
  return (
    <>
      <Button
        variant="ghost"
        aria-label={name}
        aria-haspopup="dialog"
        onClick={() => {
          setOpenings(openings + 1);
          dialog.current?.showModal();
        }}
      >
        {confirmation.action}
      </Button>
      <ConfirmModal ref={dialog} openings={openings} destructive {...confirmation}>
        {/* Only once opened, so what they fetch waits until someone asks. */}
        {openings > 0 && confirmation.children}
      </ConfirmModal>
    </>
  );
}
