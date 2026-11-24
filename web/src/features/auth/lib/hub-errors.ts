import { ConnectError } from "@connectrpc/connect";

import { RefusalReason, RefusalSchema } from "@/gen/nama/hub/v1/refusal_pb";
import type { FormErrors } from "@/shared/lib/forms";

/** The reason the Hub's Refusal on `error` names, if it carries one (ADR 0019). */
export function hubRefusalReason(error: unknown) {
  return hubRefusalOf(error)?.reason;
}

/**
 * Why the Hub refused to sign an Account in, create one or email a Sign-in Link: under the field
 * it's about, or for the whole form.
 */
export function hubErrors(error: unknown): FormErrors {
  const refusal = hubRefusalOf(error);
  const say = refusal && wording[refusal.reason];
  return say
    ? say((key) => refusal.params[key] ?? "")
    : { form: "The Hub couldn’t do that just now. Try again." };
}

// web/AGENTS.md, "Read refusals by reason": `invalid_argument` is the email's or the password's.
const wording: Partial<Record<RefusalReason, (param: (key: string) => string) => FormErrors>> = {
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

/** `seconds` rounded up to whole minutes, in words. */
function minutes(seconds: string) {
  const count = Math.ceil(Number(seconds) / 60);
  return count === 1 ? "a minute" : `${count} minutes`;
}

function hubRefusalOf(error: unknown) {
  return ConnectError.from(error).findDetails(RefusalSchema)[0];
}
