import { Code } from "@connectrpc/connect";

import { type FormErrors, refusal, refusedFieldErrors } from "@/shared/lib/forms";

/** Why the Server refused to set or clear the Switch PIN, under its field or for the whole form. */
export function switchPinRefused(error: unknown): Pick<FormErrors, "form" | "switchPin"> {
  return refusal(error, "change your Switch PIN", {
    [Code.InvalidArgument]: refusedFieldErrors(error, ["switchPin"]),
  });
}
