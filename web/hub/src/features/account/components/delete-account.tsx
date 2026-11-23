import { DeleteAccountForm } from "@hub/features/account/components/delete-account-form";
import { useDeleteAccount } from "@hub/features/account/data/sessions";
import { Button } from "nama-ui/components/button";
import { Modal } from "nama-ui/components/modal";
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
      <Modal ref={dialog} id={id}>
        <DeleteAccountForm id={id} remove={remove} close={() => dialog.current?.close()} />
      </Modal>
    </>
  );
}
