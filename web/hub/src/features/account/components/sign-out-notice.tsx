import { SignInAgain } from "@hub/features/account/components/sign-in-again";
import { refusalReason } from "@hub/features/account/lib/refusals";
import { RefusalReason } from "@hub/gen/nama/hub/v1/refusal_pb";

/** Why a session couldn't be signed out, if it couldn't, with the way back from a sign-in too old. */
export function SignOutNotice({ error }: { error: Error | null }) {
  if (refusalReason(error) === RefusalReason.RECENT_AUTHENTICATION_REQUIRED) {
    return <SignInAgain error={error} toDo="to sign out your other sessions" />;
  }
  return (
    <p role="alert" className="text-sm text-destructive empty:hidden">
      {error ? "The Hub couldn’t sign that out just now. Try again." : undefined}
    </p>
  );
}
