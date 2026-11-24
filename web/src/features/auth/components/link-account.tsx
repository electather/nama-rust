import type { Account } from "@/gen/nama/hub/v1/account_pb";

/**
 * The `account` a Sign-in Link signs in, and what else signing in does: it signs out `signedIn`,
 * the Account this browser is signed in to, if another, and takes over an Account whose email
 * wasn't `emailProven` (ADR 0100).
 */
export function LinkAccount({
  account,
  emailProven,
  signedIn,
}: {
  account: Account;
  emailProven: boolean;
  signedIn: Account | null;
}) {
  return (
    <>
      <dl className="flex flex-col gap-1">
        <dt className="text-xs text-muted-foreground">Account</dt>
        <dd className="text-base break-all">{account.email}</dd>
      </dl>
      {signedIn && signedIn.id !== account.id && (
        <p className="text-sm text-pretty text-muted-foreground">
          You’re signed in as <span className="break-all">{signedIn.email}</span>. Signing in here
          signs you out of it.
        </p>
      )}
      {!emailProven && (
        <p className="text-sm text-pretty">
          This proves the email is yours. As it wasn’t proven before, signing in also signs this
          Account out everywhere else and removes its password.
        </p>
      )}
    </>
  );
}
