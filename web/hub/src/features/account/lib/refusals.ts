import { ConnectError } from "@connectrpc/connect";
import { RefusalReason, RefusalSchema } from "@hub/gen/nama/hub/v1/refusal_pb";

/** Why a form for an email or a password was refused: under one of its fields, or for the whole form. */
export interface CredentialErrors {
  email?: string;
  password?: string;
  form?: string;
}

/** The reason the Hub's Refusal names, if `error` carries one (ADR 0019). */
export function refusalReason(error: unknown) {
  return refusalOf(error)?.reason;
}

/** What to say about a refused attempt to create an Account, sign in or set a password, by its reason. */
export function credentialErrors(error: unknown): CredentialErrors {
  return sayRefusal(error, wording, { form: "The Hub couldn’t do that just now. Try again." });
}

/** What a screen says for each reason it words, given the Refusal's params by key. */
export type RefusalWording<T> = Partial<
  Record<RefusalReason, (param: (key: string) => string) => T>
>;

/** What `wording` says about the Refusal in `error`, or `fallback` for one it doesn't word. */
export function sayRefusal<T>(error: unknown, wording: RefusalWording<T>, fallback: T): T {
  const refusal = refusalOf(error);
  const say = refusal && wording[refusal.reason];
  return say ? say((key) => refusal.params[key] ?? "") : fallback;
}

const wording: RefusalWording<CredentialErrors> = {
  // The Hub doesn’t say which of the two is wrong, so neither do we.
  [RefusalReason.WRONG_EMAIL_OR_PASSWORD]: () => ({
    password: "That email and password don’t match an Account.",
  }),
  [RefusalReason.TOO_MANY_ATTEMPTS]: (param) => ({
    form: `There have been too many tries. Try again in ${Math.ceil(Number(param("retryAfterSeconds")) / 60)} min.`,
  }),
  [RefusalReason.EMAIL_TAKEN]: () => ({
    email: "An Account already has this email. Sign in instead.",
  }),
  [RefusalReason.EMAIL_INVALID]: () => ({
    email: "Enter an email address, such as you@example.com.",
  }),
  [RefusalReason.SIGN_IN_LINKS_OFF]: () => ({
    form: "This Hub can’t email Sign-in Links. Sign in with your password instead.",
  }),
  [RefusalReason.RECENT_AUTHENTICATION_REQUIRED]: (param) => ({
    form: `You signed in over ${param("recentMinutes")} minutes ago.`,
  }),
  [RefusalReason.PASSWORD_LENGTH]: (param) => ({
    password: `A password needs ${param("minLength")} to ${param("maxLength")} characters.`,
  }),
};

function refusalOf(error: unknown) {
  return ConnectError.from(error).findDetails(RefusalSchema)[0];
}
