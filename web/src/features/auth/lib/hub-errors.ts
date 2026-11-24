import { ConnectError } from "@connectrpc/connect";

import { RefusalReason, RefusalSchema } from "@/gen/nama/hub/v1/refusal_pb";
import type { FormErrors } from "@/shared/lib/forms";

/** What an Account with no User on the Server it signs in to is told. */
export const NO_USER_ON_SERVER =
  "Your Account has no User on this Server. Ask whoever runs it to create one from your Account.";

/** What a sign-in to a Server that expired or was used says. */
export const SERVER_SIGN_IN_EXPIRED =
  "This sign-in expired or was used. Start signing in again on your Server.";

/** The reason the Hub's Refusal on `error` names, if it carries one (ADR 0019). */
export function hubRefusalReason(error: unknown) {
  return hubRefusalOf(error)?.reason;
}

/**
 * Why the Hub refused to sign an Account in, create one or email a Sign-in Link: under the field
 * it's about, or for the whole form.
 */
export function hubErrors(error: unknown): FormErrors {
  return sayHubRefusal(error, wording, "The Hub couldn’t do that just now. Try again.");
}

/** Why the Hub refused to approve a sign-in to a Server. */
export function serverSignInErrors(error: unknown): FormErrors {
  return sayHubRefusal(
    error,
    serverSignInWording,
    "The Hub couldn’t sign you in just now. Try again.",
  );
}

/** What a form says for each reason it words, given the Refusal's params by key. */
export type HubWording = Partial<
  Record<RefusalReason, (param: (key: string) => string) => FormErrors>
>;

/** What `wording` says about the Hub's Refusal on `error`, or `fallback` for the whole form. */
export function sayHubRefusal(error: unknown, wording: HubWording, fallback: string): FormErrors {
  const refusal = hubRefusalOf(error);
  const say = refusal && wording[refusal.reason];
  return say ? say((key) => refusal.params[key] ?? "") : { form: fallback };
}

// web/AGENTS.md, "Read refusals by reason": `invalid_argument` is the email's or the password's.
const wording: HubWording = {
  // The Hub doesn’t say which of the two is wrong, so neither do we.
  [RefusalReason.WRONG_EMAIL_OR_PASSWORD]: () => ({
    password: "That email and password don’t match an Account.",
  }),
  [RefusalReason.TOO_MANY_ATTEMPTS]: (param) => ({
    wait: `There have been too many tries. Try again in ${minutes(param("retryAfterSeconds"))}.`,
  }),
  [RefusalReason.EMAIL_TAKEN]: () => ({ email: "This email has an Account already. Sign in." }),
  [RefusalReason.EMAIL_INVALID]: () => ({ email: "Enter an email, such as you@example.com." }),
  [RefusalReason.SIGN_IN_LINKS_OFF]: () => ({
    form: "This Hub can’t email Sign-in Links. Sign in with your password.",
  }),
  [RefusalReason.PASSWORD_LENGTH]: (param) => ({
    password: `A password needs ${param("minLength")} to ${param("maxLength")} characters.`,
  }),
};

const serverSignInWording: HubWording = {
  [RefusalReason.SERVER_SIGN_IN_NOT_FOUND]: () => ({ form: SERVER_SIGN_IN_EXPIRED }),
  [RefusalReason.NO_USER_ON_SERVER]: () => ({ form: NO_USER_ON_SERVER }),
  // Shown only until `useApproveServerSignIn` has sent the Account to sign in.
  [RefusalReason.SIGN_IN_REQUIRED]: () => ({
    form: "You’re signed out. Sign in again to continue.",
  }),
};

/** `seconds` rounded up to whole minutes, in words. */
function minutes(seconds: string) {
  const count = Math.ceil(Number(seconds) / 60);
  return count === 1 ? "a minute" : `${count} minutes`;
}

function hubRefusalOf(error: unknown) {
  return ConnectError.from(error).findDetails(RefusalSchema)[0];
}
