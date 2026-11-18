import { Code } from "@connectrpc/connect";

import { type FormErrors, refusedFieldErrors, refusal } from "@/shared/lib/forms";

/** Why making an Invite was refused, if it was, next to the field it's about. */
export function createInviteErrors(error: unknown): FormErrors {
  if (error === null) {
    return {};
  }
  return refusal(error, "make the Invite", {
    [Code.InvalidArgument]: refusedFieldErrors(error, ["note"]),
  });
}

/** Why revoking an Invite was refused, as a sentence. */
export function revokeInviteRefusal(error: unknown) {
  return refusal(error, "revoke this Invite", {
    // Accepted or revoked meanwhile, it's no longer pending anyway.
    [Code.NotFound]: { form: "This Invite was already used or revoked." },
  }).form;
}

/** Why joining with an Invite's link was refused, next to the field it's about. */
export function acceptInviteErrors(error: unknown): FormErrors {
  return refusal(error, "join this Server", {
    [Code.NotFound]: {
      form: "This Invite no longer works: it was used, revoked or has expired. Ask whoever sent it for a new one.",
    },
    [Code.AlreadyExists]: { name: "Someone already has that name. Pick another." },
    [Code.InvalidArgument]: refusedFieldErrors(error, ["name", "password"]),
  });
}
