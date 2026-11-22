import { refusalReason } from "@hub/features/account/lib/refusals";
import { RefusalReason } from "@hub/gen/nama/hub/v1/refusal_pb";

/** What an Account with no User on the Server it signs in to is told. */
export const NO_USER_ON_SERVER =
  "Your Account has no User on this Server. Ask whoever runs it to create one from your Account.";

/** What to say about a sign-in to a Server the Hub refused, read by its reason. */
export function serverSignInError(error: unknown) {
  const reason = refusalReason(error);
  return (
    (reason === undefined ? undefined : wording[reason]) ??
    "The Hub couldn’t sign you in just now. Try again."
  );
}

/** What each reason a sign-in to a Server can be refused for says. */
const wording: Partial<Record<RefusalReason, string>> = {
  [RefusalReason.SERVER_SIGN_IN_NOT_FOUND]:
    "This sign-in expired or was used. Start signing in again on your Server.",
  [RefusalReason.NO_USER_ON_SERVER]: NO_USER_ON_SERVER,
};
