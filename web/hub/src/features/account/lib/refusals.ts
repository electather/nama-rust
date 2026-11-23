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

/**
 * What to say about a passkey that couldn't be added, removed or signed in with: nothing, `""`,
 * when the person cancelled the browser's prompt.
 */
export function passkeyError(error: unknown): string {
  if (error instanceof DOMException) {
    return browserSays(error);
  }
  const refusal = refusalOf(error);
  const say = refusal && passkeyWording[refusal.reason];
  return say
    ? say((key) => refusal.params[key] ?? "")
    : "The Hub couldn’t do that just now. Try again.";
}

/** What the browser's passkey prompt failing says, by the `DOMException`'s name. */
const browserWording: Partial<Record<string, string>> = {
  // Cancelled, or left until it timed out: nothing went wrong, so nothing is said.
  NotAllowedError: "",
  AbortError: "",
  // A passkey `excludeCredentials` lists.
  InvalidStateError: "This passkey is on your Account already.",
};

function browserSays({ name }: DOMException) {
  return browserWording[name] ?? "This browser couldn’t use a passkey here.";
}

/** What each reason a passkey can be refused for says, given the Refusal's params by key. */
const passkeyWording: Partial<Record<RefusalReason, (param: (key: string) => string) => string>> = {
  [RefusalReason.PASSKEYS_OFF]: () => "This Hub isn’t set up for passkeys.",
  [RefusalReason.PASSKEY_NOT_ACCEPTED]: () => "The Hub didn’t accept that passkey.",
  [RefusalReason.TOO_MANY_ATTEMPTS]: (param) =>
    `There have been too many tries. Try again in ${Math.ceil(Number(param("retryAfterSeconds")) / 60)} min.`,
  [RefusalReason.PASSKEY_NAME_LENGTH]: (param) =>
    `Give it a name of up to ${param("maxLength")} characters.`,
  [RefusalReason.PASSKEY_NOT_FOUND]: () => "That passkey was removed already.",
  [RefusalReason.LAST_SIGN_IN_METHOD]: () =>
    "This passkey is your only way to sign in, so it can’t be removed.",
};

function refusalOf(error: unknown) {
  return ConnectError.from(error).findDetails(RefusalSchema)[0];
}
