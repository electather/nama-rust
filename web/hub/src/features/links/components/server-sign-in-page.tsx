import { ServerSignInAs } from "@hub/features/links/components/server-sign-in-as";
import { useServerSignIn } from "@hub/features/links/data/server-sign-ins";
import { serverSignInError } from "@hub/features/links/lib/server-sign-in-refusal";

/**
 * Signing in to a Server with the signed-in Account, which the Server sent the browser here for:
 * as which User, and approving it, which sends the browser back.
 */
export function ServerSignInPage({ id }: { id: string }) {
  const signIn = useServerSignIn(id);
  if (signIn.isPending) {
    return <p className="text-center text-sm text-muted-foreground">Looking up this sign-in…</p>;
  }
  if (signIn.isError) {
    return (
      <p role="alert" className="text-sm text-destructive">
        {serverSignInError(signIn.error)}
      </p>
    );
  }
  return <ServerSignInAs id={id} {...signIn.data} />;
}
