import { Code } from "@connectrpc/connect";
import { useLocation } from "@tanstack/react-router";
import type { RefObject } from "react";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { StartAccountSignIn } from "@/shared/components/account-sign-in/start-account-sign-in";
import { ConfirmModal } from "@/shared/components/confirm-dialog/confirm-modal";
import { PasswordField } from "@/shared/components/form-field/password-field";
import { useAccountSignIn } from "@/shared/hooks/use-account-sign-in";
import { useReauthenticate } from "@/shared/hooks/use-reauthenticate";
import { accountReturnUrl } from "@/shared/lib/account-sign-in";
import { formText, refusal, serverMessage } from "@/shared/lib/forms";

/**
 * Asks for the User's password, and gives this Device Recent Authentication with it. On a Server
 * Linked to the Hub it also offers their Account, the one way in for a User with no password,
 * which comes back to this page to make the change again.
 */
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
  const here = useLocation().href;
  const offersAccount = useAccountSignIn() === true;
  const howToConfirm = offersAccount
    ? "Enter your password, or sign in with your Account,"
    : "Enter your password";
  return (
    <ConfirmModal
      ref={dialog}
      openings={openings}
      onClose={onClose}
      action="Confirm"
      title="Confirm it’s you"
      description={
        confirming === undefined
          ? `${howToConfirm} to go on. This keeps anyone else using this Device from making the change.`
          : `${howToConfirm} to ${confirming}. This keeps anyone else using this Device from doing it.`
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
      {offersAccount && (
        <StartAccountSignIn returnUrl={accountReturnUrl("/reauthenticate/account", here)} />
      )}
    </ConfirmModal>
  );
}
