import { type HubWording, sayHubRefusal } from "@/features/auth/lib/hub-errors";
import { RefusalReason } from "@/gen/nama/hub/v1/refusal_pb";
import type { FormErrors } from "@/shared/lib/forms";

/** Why the Hub refused a code that Links a Server or a User: under the code, or for the form. */
export function linkErrors(error: unknown): FormErrors {
  return sayHubRefusal(error, wording, "The Hub couldn’t Link it just now. Try again.");
}

const wording: HubWording = {
  [RefusalReason.LINK_CODE_NOT_FOUND]: () => ({
    linkCode:
      "Nothing is waiting on this code. Check it, or get a new one in the Server’s Settings.",
  }),
  [RefusalReason.USER_LINKED]: () => ({
    form: "This User is Linked to an Account already. Unlink them in their Server’s Settings first.",
  }),
  [RefusalReason.ACCOUNT_HAS_USER_ON_SERVER]: () => ({
    form: "Your Account has a User on this Server already, and it can have only one there. Unlink that User first.",
  }),
  [RefusalReason.EMAIL_NOT_PROVEN]: () => ({
    form: "Linking needs your email proven first. Sign in with a Sign-in Link sent to it, then try again.",
  }),
  // A Link gets this, not EMAIL_NOT_PROVEN, on a Hub without mail, which can't prove an email.
  [RefusalReason.SIGN_IN_LINKS_OFF]: () => ({
    form: "This Hub can’t prove emails, so Linking isn’t possible on it. Ask whoever runs it to set up email.",
  }),
  [RefusalReason.USER_INVITE_NOT_FOUND]: () => ({
    form: "This User isn’t waiting for you any more.",
  }),
  [RefusalReason.SIGN_IN_REQUIRED]: () => ({ form: "You’re signed out. Sign in again to Link." }),
  [RefusalReason.RECENT_AUTHENTICATION_REQUIRED]: (param) => ({
    form: `To Link, sign in again: your sign-in is over ${param("recentMinutes")} minutes old.`,
  }),
};
