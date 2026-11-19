import { Code } from "@connectrpc/connect";
import type { RefObject } from "react";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { ConfirmModal } from "@/shared/components/confirm-dialog/confirm-modal";
import { PasswordField } from "@/shared/components/form-field/password-field";
import { useReauthenticate } from "@/shared/hooks/use-reauthenticate";
import { formText, refusal, serverMessage } from "@/shared/lib/forms";

/** Asks for the User's password, and gives this Device Recent Authentication with it. */
export function ReauthenticateDialog({
  dialog,
  openings,
  confirming,
  onReauthenticated,
  onClose,
}: {
  dialog: RefObject<HTMLDialogElement | null>;
  openings: number;
  /** What the password is for, such as `sign in “Laptop”`, or else it's to go on with a change. */
  confirming: string | undefined;
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
      description={
        confirming === undefined
          ? "Enter your password to go on. This keeps anyone else using this Device from making the change."
          : `Enter your password to ${confirming}. This keeps anyone else using this Device from doing it.`
      }
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
      <PasswordField label="Password" autoComplete="current-password" />
    </ConfirmModal>
  );
}
