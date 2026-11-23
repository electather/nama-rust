import { SignInAgain } from "@hub/features/account/components/sign-in-again";
import { refusalReason } from "@hub/features/account/lib/refusals";
import { RefusalReason } from "@hub/gen/nama/hub/v1/refusal_pb";

/** Why the Hub didn't delete the Account, with the way back from a sign-in too old. */
export function DeleteAccountNote({ error }: { error: Error | null }) {
  if (refusalReason(error) === RefusalReason.RECENT_AUTHENTICATION_REQUIRED) {
    return <SignInAgain error={error} toDo="to delete your Account" />;
  }
  return (
    <p role="alert" className="text-sm text-destructive empty:hidden">
      {error ? "The Hub couldn’t delete your Account just now. Try again." : undefined}
    </p>
  );
}
