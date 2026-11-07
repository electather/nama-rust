import { Code } from "@connectrpc/connect";
import { createConnectQueryKey } from "@connectrpc/connect-query";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { UserService } from "@/gen/nama/v1/user_pb";

import { type FormErrors, refusedFieldErrors, refusal, serverMessage } from "./forms";

/** The query key of the Users, to refetch after a change to them. */
export const usersKey = createConnectQueryKey({
  schema: UserService.method.listUsers,
  cardinality: "finite",
});

/** The query key of every User's Devices, to refetch after a change that signs some out. */
export const devicesKey = createConnectQueryKey({
  schema: UserService.method.listDevices,
  cardinality: "finite",
});

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

/** Why the Server refused to change your own password, next to the field it's about. */
export function changePasswordErrors(error: unknown): FormErrors {
  return refusal(error, "change your password", {
    [Code.InvalidArgument]: refusedFieldErrors(error, ["newPassword"]),
    // This Device was signed out meanwhile.
    [Code.Unauthenticated]: { form: serverMessage(error) },
  });
}

/** Why the Server refused a new password an Admin set for the User called `name`. */
export function resetPasswordRefusal(error: unknown, name: string) {
  return refusal(error, "reset the password", {
    [Code.InvalidArgument]: { form: serverMessage(error) },
    [Code.NotFound]: noLongerHere(name),
  }).form;
}

/** What to say about the User called `name` once the Server no longer has them. */
export function noLongerHere(name: string) {
  return { form: `${name} isn’t on this Server any more.` };
}
