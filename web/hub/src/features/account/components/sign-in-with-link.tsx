import { SignedInElsewhere } from "@hub/features/account/components/signed-in-elsewhere";
import type { Account } from "@hub/gen/nama/hub/v1/account_pb";
import { Button } from "nama-ui/components/button";

/** Who the link signs in, what else signing in does, and the button that does it. */
export function SignInWithLink({
  account,
  emailProven,
  pending,
  failed,
  onSignIn,
}: {
  account: Account;
  /** If not, signing in takes the Account over (ADR 0100). */
  emailProven: boolean;
  pending: boolean;
  failed: boolean;
  onSignIn: () => void;
}) {
  return (
    <>
      <h1 className="text-center text-3xl font-medium tracking-tight text-balance">Sign in</h1>
      <dl className="flex flex-col gap-1">
        <dt className="text-xs text-muted-foreground">Account</dt>
        <dd className="text-base break-all">{account.email}</dd>
      </dl>
      <SignedInElsewhere accountId={account.id} />
      {emailProven ? undefined : (
        <p className="text-sm text-pretty">
          This proves the email is yours. As it wasn’t proven before, signing in also signs this
          Account out everywhere else and removes its password.
        </p>
      )}
      <p role="alert" className="text-sm text-destructive empty:hidden">
        {failed ? "The Hub couldn’t sign you in just now. Try again." : undefined}
      </p>
      <Button size="lg" disabled={pending} onClick={onSignIn}>
        {pending ? "Signing in…" : "Sign in"}
      </Button>
    </>
  );
}
