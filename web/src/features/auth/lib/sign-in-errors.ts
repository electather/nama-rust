import { Code } from "@connectrpc/connect";

import { type FormErrors, refusal, refusedFieldErrors, serverMessage } from "@/shared/lib/forms";

/** The refusals of signing in with a password, as errors under its fields. */
export function signInErrors(error: unknown): FormErrors {
  return refusal(error, "sign in", {
    // The Server doesn’t say whether it was the username or the password, so neither do we.
    [Code.Unauthenticated]: { password: "That username and password don’t match." },
  });
}

/** The refusals of setting up the Server, as errors under its fields. */
export function setupErrors(error: unknown): FormErrors {
  return refusal(error, "set up this Server", {
    [Code.PermissionDenied]: {
      setupCode: "That isn’t the Setup Code. Check the Server’s log for it.",
    },
    [Code.FailedPrecondition]: {
      form: "This Server is already set up. Reload the page to sign in.",
    },
    [Code.InvalidArgument]: refusedFieldErrors(error, ["name", "password"]),
  });
}

/** The refusals of asking for a Sign-in Code, as errors under its fields. */
export function startErrors(error: unknown): FormErrors {
  return refusal(error, "get a code", {
    [Code.InvalidArgument]: { deviceName: serverMessage(error) },
    // The Server caps how many sign-ins wait at once; each code expires within 10 minutes.
    [Code.ResourceExhausted]: {
      form: "Too many Devices are waiting to sign in. Try again in a few minutes.",
    },
  });
}

/** The refusals of looking up or approving a Sign-in Code, as errors under its field. */
export function codeErrors(error: unknown): FormErrors {
  return refusal(error, "check that code", {
    [Code.NotFound]: {
      signInCode:
        "That code doesn’t match a Device waiting to sign in. Check it, or get a new code on the Device.",
    },
    [Code.FailedPrecondition]: { signInCode: "That code has already been used." },
  });
}
