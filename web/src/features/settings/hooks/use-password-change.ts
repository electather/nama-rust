import { useState } from "react";

import { useChangePassword } from "@/features/settings/data/use-change-password";
import { useConfirmPassword } from "@/features/settings/data/use-confirm-password";
import { type PasswordErrors, gaps, refused } from "@/features/settings/lib/password-change";
import { formText } from "@/shared/lib/forms";

const fields = ["password", "newPassword", "confirm"] as const;

/** Puts the cursor in the first field of `form` that `errors` is about. */
function focusFirst(form: HTMLFormElement, errors: PasswordErrors) {
  const name = fields.find((field) => errors[field] !== undefined);
  const field = name === undefined ? null : form.elements.namedItem(name);
  if (field instanceof HTMLInputElement) {
    field.focus();
  }
}

/**
 * Changing the signed-in User's own password from the Sign-in pane. The current password typed
 * there gives this Device Recent Authentication, so the change isn't asked about a second time.
 */
export function usePasswordChange() {
  const reauthenticate = useConfirmPassword();
  const change = useChangePassword();
  const [errors, setErrors] = useState<PasswordErrors>({});
  const [done, setDone] = useState(false);

  async function submit(typed: FormData, form: HTMLFormElement) {
    setDone(false);
    const missing = gaps(typed);
    if (Object.keys(missing).length > 0) {
      setErrors(missing);
      focusFirst(form, missing);
      return;
    }
    try {
      await reauthenticate.mutateAsync({
        proof: { case: "password", value: formText(typed, "password") },
      });
      await change.mutateAsync({ newPassword: formText(typed, "newPassword") });
    } catch (error) {
      const why = refused(error);
      setErrors(why);
      focusFirst(form, why);
      return;
    }
    form.reset();
    setErrors({});
    setDone(true);
  }

  return { errors, done, pending: reauthenticate.isPending || change.isPending, submit };
}
