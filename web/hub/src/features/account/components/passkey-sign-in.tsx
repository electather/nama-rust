import { PasskeyNotice } from "@hub/features/account/components/passkey-notice";
import { useCanSignInWithPasskey, useSignInWithPasskey } from "@hub/features/account/data/passkeys";
import { Button } from "nama-ui/components/button";

/**
 * Signing in with a passkey, with no email or password, then going on to the Hub path
 * `continueTo`, on a Hub that offers passkeys and in a browser that has them.
 */
export function PasskeySignIn({ continueTo }: { continueTo: string }) {
  const signIn = useSignInWithPasskey(continueTo);
  if (!useCanSignInWithPasskey()) {
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
