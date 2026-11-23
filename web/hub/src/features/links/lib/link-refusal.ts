import { refusalReason } from "@hub/features/account/lib/refusals";
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

/**
 * What to say about a code the Hub refused, read by its reason; nothing before it refused one.
 * Without `sendsSignInLinks`, nothing on the Hub can prove an email, so none is offered.
 */
export function linkErrors(error: unknown, sendsSignInLinks: boolean): LinkErrors {
  const errors = refusalErrors(error);
  return errors.proveEmail === true && !sendsSignInLinks ? cannotProveEmail : errors;
}

function refusalErrors(error: unknown): LinkErrors {
  if (error === null) {
    return {};
  }
  const reason = refusalReason(error);
  return (
    (reason === undefined ? undefined : wording[reason]) ?? {
      form: "The Hub couldn’t Link it just now. Try again.",
    }
  );
}

/** The code typed in `form`. */
export function typedCode(form: HTMLFormElement) {
  const code = new FormData(form).get("code");
  return typeof code === "string" ? code : "";
}

const cannotProveEmail: LinkErrors = {
  form: "This Hub can’t prove emails, so Linking isn’t possible on it. Ask whoever runs it to set up email.",
};

/** What each reason a code can be refused for says. */
const wording: Partial<Record<RefusalReason, LinkErrors>> = {
  [RefusalReason.LINK_CODE_NOT_FOUND]: {
    code: "Nothing is waiting on this code. Check it, or get a new one in the Server’s Settings.",
  },
  [RefusalReason.USER_LINKED]: {
    form: "This User is Linked to an Account already. Unlink them in their Server’s Settings first.",
  },
  [RefusalReason.EMAIL_NOT_PROVEN]: {
    form: "Linking needs your email proven first. Sign in with a Sign-in Link sent to it, then try again.",
    proveEmail: true,
  },
  [RefusalReason.USER_INVITE_NOT_FOUND]: {
    form: "This User isn’t waiting for you any more. Reload the page.",
  },
  [RefusalReason.SIGN_IN_REQUIRED]: {
    form: "You’re signed out. Sign in again to Link.",
    signInAgain: true,
  },
  [RefusalReason.RECENT_AUTHENTICATION_REQUIRED]: {
    form: "To Link, sign in again: your sign-in is over 10 minutes old.",
    signInAgain: true,
  },
};
