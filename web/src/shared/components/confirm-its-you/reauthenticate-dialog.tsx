import { Code } from "@connectrpc/connect";
import type { RefObject } from "react";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { ConfirmModal } from "@/shared/components/confirm-dialog/confirm-modal";
import { FormField } from "@/shared/components/form-field/form-field";
import { useReauthenticate } from "@/shared/hooks/use-reauthenticate";
import { formText, refusal, serverMessage } from "@/shared/lib/forms";

/** Asks for the User's password, and gives this Device Recent Authentication with it. */
export function ReauthenticateDialog({
  dialog,
  openings,
  onReauthenticated,
  onClose,
}: {
  dialog: RefObject<HTMLDialogElement | null>;
  openings: number;
  onReauthenticated: () => void;
  onClose: () => void;
}) {
  const reauthenticate = useReauthenticate(onReauthenticated);
  return (
    <ConfirmModal
      ref={dialog}
      openings={openings}
      onClose={onClose}
      action="Confirm"
      title="Confirm it’s you"
      description="Enter your password to go on. This keeps anyone else using this Device from making the change."
      onConfirm={async (typed) => {
        await reauthenticate.mutateAsync({
          proof: { case: "password", value: formText(typed, "password") },
        });
      }}
      refusal={(error) =>
        refusal(
          error,
          "confirm it’s you",
          // This Device was signed out meanwhile.
          { [Code.Unauthenticated]: { form: serverMessage(error) } },
          {
            [RefusalReason.WRONG_PASSWORD]: { form: "That isn’t your password." },
          },
        ).form
      }
    >
      <FormField label="Password" name="password" type="password" autoComplete="current-password" />
    </ConfirmModal>
  );
}
