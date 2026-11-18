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
        form: `${name} is the only Admin. Make someone else an Admin first.`,
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

/** Why the Server refused a new password an Admin set for the User called `name`. */
export function resetPasswordRefusal(error: unknown, name: string) {
  return refusal(error, "reset the password", {
    [Code.InvalidArgument]: { form: serverMessage(error) },
    [Code.NotFound]: noLongerHere(name),
  }).form;
}
