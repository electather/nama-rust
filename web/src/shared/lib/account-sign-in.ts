import { Code } from "@connectrpc/connect";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { type FormErrors, refusal, serverMessage } from "@/shared/lib/forms";

// sessionStorage keeps it to this tab, which goes to the Hub and comes back to the page finishing it.
const HANDLE_KEY = "nama.account-sign-in";

/** Keeps the secret that finishes the sign-in with an Account this tab started, or confirms it’s you. */
export function keepAccountSignIn(handle: string) {
  sessionStorage.setItem(HANDLE_KEY, handle);
}

/** The secret kept, forgotten as it's read: it works once. */
export function takeAccountSignIn() {
  const handle = sessionStorage.getItem(HANDLE_KEY) ?? "";
  sessionStorage.removeItem(HANDLE_KEY);
  return handle;
}

/** Where the Hub sends this browser back to once the Account approves: `page`, keeping `redirect`. */
export function accountReturnUrl(page: string, redirect: string | undefined) {
  const url = new URL(page, globalThis.location.origin);
  if (redirect !== undefined) {
    url.searchParams.set("redirect", redirect);
  }
  return url.href;
}

/** The refusals of signing in, or confirming it’s you, with an Account, for the whole form. */
export function accountSignInErrors(error: unknown): FormErrors {
  return refusal(
    error,
    "sign in with your Account",
    // The Server says the Hub can't be reached, and what to do.
    { [Code.Unavailable]: { form: serverMessage(error) } },
    {
      [RefusalReason.ACCOUNT_SIGN_IN_NOT_FOUND]: {
        form: "The Hub didn’t approve this sign-in, or it expired. Try again.",
      },
      [RefusalReason.WRONG_ACCOUNT]: {
        form: "Another Account approved this. Sign in to the Hub with the Account Linked to you, and try again.",
      },
      [RefusalReason.USER_NOT_FOUND]: {
        form: "Your User on this Server was deleted. Ask whoever runs it to add you again.",
      },
      [RefusalReason.NO_SERVER_LINKER]: {
        form: "This Server isn’t Linked to the Hub any more. Ask whoever runs it to Link it again.",
      },
    },
  );
}
