import { Code } from "@connectrpc/connect";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { type FormErrors, refusedFieldErrors, refusal, serverMessage } from "@/shared/lib/forms";
import { noLongerHere } from "@/shared/lib/no-longer-here";

/**
 * Why the Server refused to `attempt` (such as "delete this User") a change to the User called
 * `name`: one that would leave it with no Admin, or to a User deleted meanwhile.
 */
export function onlyAdminRefusal(error: unknown, name: string, attempt: string) {
  return refusal(
    error,
    attempt,
    { [Code.NotFound]: noLongerHere(name) },
    {
      [RefusalReason.LAST_ADMIN]: {
        form: `${name} is the only Admin who can sign in with a password. Make another User with a password an Admin first.`,
      },
    },
  ).form;
}

/** Why creating a User was refused, next to the field it's about. */
export function createUserErrors(error: unknown): FormErrors {
  return refusal(error, "create the User", {
    [Code.AlreadyExists]: { name: "Another User already has that name." },
    [Code.InvalidArgument]: refusedFieldErrors(error, ["name", "password"]),
  });
}

/** Why creating a User from an Account was refused, next to the field it's about. */
export function createFromAccountErrors(error: unknown): FormErrors {
  return refusal(
    error,
    "create the User",
    {
      [Code.AlreadyExists]: { name: "Another User already has that name." },
      [Code.InvalidArgument]: refusedFieldErrors(error, ["name"]),
      // The Server says the Hub couldn't be asked, and that no User was created.
      [Code.Unavailable]: { form: serverMessage(error) },
    },
    {
      [RefusalReason.NO_SERVER_LINKER]: {
        form: "This Server isn’t Linked to the Hub any more, so no User was created.",
      },
    },
  );
}

/** Why the Server refused a new password an Admin set for the User called `name`. */
export function resetPasswordRefusal(error: unknown, name: string) {
  return refusal(error, "reset the password", {
    [Code.InvalidArgument]: { form: serverMessage(error) },
    [Code.NotFound]: noLongerHere(name),
  }).form;
}

/** Why the Server refused to clear the Switch PIN of the User called `name`. */
export function clearSwitchPinRefusal(error: unknown, name: string) {
  return refusal(
    error,
    "clear the Switch PIN",
    { [Code.NotFound]: noLongerHere(name) },
    // The Server asks for this only to clear an Admin's.
    {
      [RefusalReason.PERMISSION_REQUIRED]: {
        form: "Only an Admin can clear an Admin’s Switch PIN.",
      },
    },
  ).form;
}
