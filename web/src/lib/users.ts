import { Code } from "@connectrpc/connect";
import { createConnectQueryKey } from "@connectrpc/connect-query";

import { UserService } from "@/gen/nama/v1/user_pb";
import { type FormErrors, refusedFieldErrors, refusal, serverMessage } from "@/shared/lib/forms";

/** The query key of the pending Invites, to refetch after making or revoking one. */
export const invitesKey = createConnectQueryKey({
  schema: UserService.method.listInvites,
  cardinality: "finite",
});

/** The query key of every User's Devices, to refetch after a change that signs some out. */
export const devicesKey = createConnectQueryKey({
  schema: UserService.method.listDevices,
  cardinality: "finite",
});

/** Why the Server refused to change your own password, next to the field it's about. */
export function changePasswordErrors(error: unknown): FormErrors {
  return refusal(error, "change your password", {
    [Code.InvalidArgument]: refusedFieldErrors(error, ["newPassword"]),
    // This Device was signed out meanwhile.
    [Code.Unauthenticated]: { form: serverMessage(error) },
  });
}

/** What to say about the User called `name` once the Server no longer has them. */
export function noLongerHere(name: string) {
  return { form: `${name} isn’t on this Server any more.` };
}
