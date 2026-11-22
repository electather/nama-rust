import { useAccount } from "@hub/features/account/data/account";

/** Says signing in signs out the Account this browser is signed in to, if it isn't `accountId`. */
export function SignedInElsewhere({ accountId }: { accountId: string }) {
  const signedIn = useAccount();
  if (!signedIn || signedIn.id === accountId) {
    return null;
  }
  return (
    <p className="text-sm text-pretty text-muted-foreground">
      You’re signed in as <span className="break-all">{signedIn.email}</span>. Signing in here signs
      you out of it.
    </p>
  );
}
