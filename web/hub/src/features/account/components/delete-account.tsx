import { DeleteAccountForm } from "@hub/features/account/components/delete-account-form";
import { useDeleteAccount } from "@hub/features/account/data/sessions";
import { Button } from "nama-ui/components/button";
import { useId, useRef } from "react";

/** Deleting the signed-in Account, once a dialog has said what goes with it and it's confirmed. */
export function DeleteAccount() {
  const dialog = useRef<HTMLDialogElement>(null);
  const remove = useDeleteAccount();
  const id = useId();
  return (
    <>
      <Button
        variant="ghost"
        size="lg"
        aria-haspopup="dialog"
        onClick={() => {
          remove.reset();
          dialog.current?.showModal();
        }}
      >
        Delete Account
      </Button>
      {/* The native modal traps focus, closes on Escape and gives focus back itself. */}
      <dialog
        ref={dialog}
        role="alertdialog"
        aria-labelledby={`${id}-title`}
        aria-describedby={`${id}-description`}
        // As the web app's confirm dialogs (DESIGN.md's Dialog): scrolling in it, or over its
        // backdrop, doesn't scroll the page behind.
        className="m-auto w-full overscroll-contain rounded-lg border bg-background text-foreground shadow-lg backdrop:overflow-hidden backdrop:overscroll-contain backdrop:bg-scrim sm:max-w-lg"
      >
        <DeleteAccountForm id={id} remove={remove} close={() => dialog.current?.close()} />
      </dialog>
    </>
  );
}
