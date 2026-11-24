import { type RefusalWording, sayRefusal } from "@hub/features/account/lib/refusals";
import { RefusalReason } from "@hub/gen/nama/hub/v1/refusal_pb";

/** Why entering a code failed: under the code, or for the whole form. */
export interface LinkErrors {
  code?: string;
  form?: string;
  /** Whether signing in again would let it through. */
  signInAgain?: boolean;
  /** Whether using a Sign-in Link, which proves the Account's email, would let it through. */
  proveEmail?: boolean;
}

/** What to say about a code the Hub refused, read by its reason; nothing before it refused one. */
export function linkErrors(error: unknown): LinkErrors {
  if (error === null) {
    return {};
  }
  return sayRefusal(error, wording, { form: "The Hub couldn’t Link it just now. Try again." });
}

/** The code typed in `form`. */
export function typedCode(form: HTMLFormElement) {
  const code = new FormData(form).get("code");
  return typeof code === "string" ? code : "";
}

/** What each reason a code can be refused for says. */
const wording: RefusalWording<LinkErrors> = {
  [RefusalReason.LINK_CODE_NOT_FOUND]: () => ({
    code: "Nothing is waiting on this code. Check it, or get a new one in the Server’s Settings.",
  }),
  [RefusalReason.USER_LINKED]: () => ({
    form: "This User is Linked to an Account already. Unlink them in their Server’s Settings first.",
  }),
  [RefusalReason.ACCOUNT_HAS_USER_ON_SERVER]: () => ({
    form: "Your Account has a User on this Server already, and it can have only one there. Unlink that User under Your Users first.",
  }),
  [RefusalReason.EMAIL_NOT_PROVEN]: () => ({
    form: "Linking needs your email proven first. Sign in with a Sign-in Link sent to it, then try again.",
    proveEmail: true,
  }),
  // On a Link, the Hub has no mail, so nothing can prove the email Linking needs.
  [RefusalReason.SIGN_IN_LINKS_OFF]: () => ({
    form: "This Hub can’t prove emails, so Linking isn’t possible on it. Ask whoever runs it to set up email.",
  }),
  [RefusalReason.USER_INVITE_NOT_FOUND]: () => ({
    form: "This User isn’t waiting for you any more. Reload the page.",
  }),
  [RefusalReason.SIGN_IN_REQUIRED]: () => ({
    form: "You’re signed out. Sign in again to Link.",
    signInAgain: true,
  }),
  [RefusalReason.RECENT_AUTHENTICATION_REQUIRED]: (param) => ({
    form: `To Link, sign in again: your sign-in is over ${param("recentMinutes")} minutes old.`,
    signInAgain: true,
  }),
};
