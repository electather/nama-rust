import { PasskeyNotice } from "@hub/features/account/components/passkey-notice";
import { useSignInWithPasskey } from "@hub/features/account/data/passkeys";
import { browserHasPasskeys } from "@hub/features/account/lib/webauthn";
import { Button } from "nama-ui/components/button";

/**
 * Signing in with a passkey, with no email or password, then going on to the Hub path
 * `continueTo`, in a browser that has passkeys.
 */
export function PasskeySignIn({ continueTo }: { continueTo: string }) {
  const signIn = useSignInWithPasskey(continueTo);
  if (!browserHasPasskeys()) {
    return null;
  }
  return (
    <>
      <Button
        type="button"
        variant="outline"
        size="lg"
        disabled={signIn.isPending}
        onClick={() => {
          signIn.mutate();
        }}
      >
        {signIn.isPending ? "Signing in…" : "Sign in with a passkey"}
      </Button>
      <PasskeyNotice error={signIn.error} />
    </>
  );
}
