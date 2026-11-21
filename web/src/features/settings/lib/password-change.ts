import { Code } from "@connectrpc/connect";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import {
  type FormErrors,
  formText,
  refusal,
  refusedFieldErrors,
  serverMessage,
} from "@/shared/lib/forms";

/** What's wrong with the password form, next to the field it's about. */
export type PasswordErrors = Pick<FormErrors, "form" | "password" | "newPassword"> & {
  confirm?: string;
};

// The shortest password the Server's own sign-in takes, which the field's hint says too.
const SHORTEST = 8;

/** What's missing from what was `typed`, before the Server is asked: `{}` when nothing is. */
export function gaps(typed: FormData): PasswordErrors {
  const next = formText(typed, "newPassword");
  const again = formText(typed, "confirm");
  return {
    ...(formText(typed, "password") === "" && { password: "Enter your current password." }),
    ...(next.length < SHORTEST && { newPassword: "Use at least 8 characters." }),
    ...(again !== next && { confirm: mismatch(again) }),
  };
}

function mismatch(again: string) {
  return again === "" ? "Type the new password again." : "This doesn’t match the new password.";
}

/** Why the Server refused to change the password, next to the field it's about. */
export function refused(error: unknown): PasswordErrors {
  return refusal(
    error,
    "change your password",
    {
      [Code.InvalidArgument]: refusedFieldErrors(error, ["newPassword"]),
      // This Device was signed out meanwhile.
      [Code.Unauthenticated]: { form: serverMessage(error) },
    },
    { [RefusalReason.WRONG_PASSWORD]: { password: "That isn’t your current password." } },
  );
}
