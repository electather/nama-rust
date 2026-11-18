import { Code } from "@connectrpc/connect";

import { type FormErrors, refusedFieldErrors, refusal, serverMessage } from "@/shared/lib/forms";

/** Why the Server refused to change your own password, next to the field it's about. */
export function changePasswordErrors(error: unknown): FormErrors {
  return refusal(error, "change your password", {
    [Code.InvalidArgument]: refusedFieldErrors(error, ["newPassword"]),
    // This Device was signed out meanwhile.
    [Code.Unauthenticated]: { form: serverMessage(error) },
  });
}
