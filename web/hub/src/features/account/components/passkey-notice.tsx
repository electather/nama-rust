import { SignInAgain } from "@hub/features/account/components/sign-in-again";
import { passkeyError, refusalReason } from "@hub/features/account/lib/refusals";
import { RefusalReason } from "@hub/gen/nama/hub/v1/refusal_pb";

/**
 * Why a passkey couldn't be added, removed or signed in with, if it couldn't, with the way back
 * from a sign-in too old.
 */
export function PasskeyNotice({ error }: { error: Error | null }) {
  if (refusalReason(error) === RefusalReason.RECENT_AUTHENTICATION_REQUIRED) {
    return <SignInAgain error={error} toDo="to change your passkeys" />;
  }
  return (
    <p role="alert" className="text-sm text-destructive empty:hidden">
      {error ? passkeyError(error) : undefined}
    </p>
  );
}
