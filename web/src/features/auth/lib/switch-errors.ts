import { Code } from "@connectrpc/connect";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { type FormErrors, refusal, refusalOf } from "@/shared/lib/forms";

/** Whether switching was refused only for want of the User's Switch PIN. */
export function needsSwitchPin(error: unknown) {
  return refusalOf(error).reason === RefusalReason.FIELD_REQUIRED;
}

/** The refusals of switching to `name`, as errors under the Switch PIN or for the whole page. */
export function switchErrors(error: unknown, name = "That User"): FormErrors {
  return refusal(
    error,
    "switch",
    {
      [Code.NotFound]: { form: `${name} isn’t signed in to this Device. They can sign in again.` },
    },
    {
      [RefusalReason.FIELD_REQUIRED]: {},
      [RefusalReason.WRONG_SWITCH_PIN]: wrongPin(error, name),
    },
  );
}

/** Why `name`'s Switch PIN was refused: how many tries are left, or that the last one signed them out. */
function wrongPin(error: unknown, name: string): FormErrors {
  const { triesLeft } = refusalOf(error).params;
  if (triesLeft === "0") {
    return {
      form: `That was the last try, so ${name} was signed out of this Device. They can sign in again.`,
    };
  }
  const left = triesLeft === "1" ? "1 try" : `${triesLeft ?? "A few"} tries`;
  return { switchPin: `That isn’t ${name}’s Switch PIN. ${left} left.` };
}
