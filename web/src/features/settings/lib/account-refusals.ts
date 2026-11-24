import {
  type HubWording,
  hubRefusalReason,
  sayHubRefusal,
  tooManyTries,
} from "@/features/auth/lib/hub-errors";
import { RefusalReason } from "@/gen/nama/hub/v1/refusal_pb";

/** Whether the Hub refused for want of Recent Authentication, which signing in again gives. */
export function needsRecentSignIn(error: unknown) {
  return hubRefusalReason(error) === RefusalReason.RECENT_AUTHENTICATION_REQUIRED;
}

/** How long ago the Account signed in, as the Hub's Recent Authentication refusal on `error` says. */
export function signedInTooLongAgo(error: unknown) {
  return sayHubRefusal(error, recentWording, "You signed in a while ago.").form;
}

const recentWording: HubWording = {
  [RefusalReason.RECENT_AUTHENTICATION_REQUIRED]: (param) => ({
    form: `You signed in over ${param("recentMinutes")} minutes ago.`,
  }),
};

/**
 * What to say about a passkey that couldn't be added or removed: nothing, `""`, when the person
 * cancelled the browser's prompt.
 */
export function passkeyError(error: unknown) {
  if (error instanceof DOMException) {
    return browserWording[error.name] ?? "This browser couldn’t use a passkey here.";
  }
  return sayHubRefusal(error, passkeyWording, "The Hub couldn’t do that just now. Try again.").form;
}

/** What the browser's passkey prompt failing says, by the `DOMException`'s name. */
const browserWording: Partial<Record<string, string>> = {
  // Cancelled, or left until it timed out: nothing went wrong, so nothing is said.
  NotAllowedError: "",
  AbortError: "",
  // A passkey `excludeCredentials` lists.
  InvalidStateError: "This passkey is on your Account already.",
};

const passkeyWording: HubWording = {
  [RefusalReason.PASSKEYS_OFF]: () => ({ form: "This Hub isn’t set up for passkeys." }),
  [RefusalReason.PASSKEY_NOT_ACCEPTED]: () => ({ form: "The Hub didn’t accept that passkey." }),
  [RefusalReason.TOO_MANY_ATTEMPTS]: (param) => ({
    form: tooManyTries(param("retryAfterSeconds")),
  }),
  [RefusalReason.PASSKEY_NAME_LENGTH]: (param) => ({
    form: `Give it a name of up to ${param("maxLength")} characters.`,
  }),
  [RefusalReason.PASSKEY_NOT_FOUND]: () => ({ form: "That passkey was removed already." }),
  [RefusalReason.LAST_SIGN_IN_METHOD]: () => ({
    form: "This passkey is your only way to sign in, so it can’t be removed.",
  }),
};
