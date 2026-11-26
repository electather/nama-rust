import { Button } from "nama-ui/components/button";

import { useFinishSignIn } from "@/features/auth/data/use-finish-sign-in";
import { useSignInWithPasskey } from "@/features/auth/data/use-sign-in-with-passkey";
import { passkeyError } from "@/features/auth/lib/hub-errors";
import { Notice } from "@/shared/components/sign-in-layout/notice";

/** Signs in at the Hub with a passkey, with no email or password, then goes on to `redirect`. */
export function PasskeyButton({ redirect }: { redirect: string | undefined }) {
  const signIn = useSignInWithPasskey(useFinishSignIn(redirect));
  const error = signIn.error ? passkeyError(signIn.error) : "";
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
      {error !== "" && <Notice kind="error">{error}</Notice>}
    </>
  );
}
