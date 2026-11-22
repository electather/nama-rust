import { credentialErrors, refusalReason } from "@hub/features/account/lib/refusals";
import { RefusalReason } from "@hub/gen/nama/hub/v1/refusal_pb";
import { Link } from "@tanstack/react-router";

/** Why the Hub refused a password, with the way back from a sign-in too old, or that it saved it. */
export function PasswordFormNote({ error, saved }: { error: Error | null; saved: boolean }) {
  if (refusalReason(error) === RefusalReason.RECENT_AUTHENTICATION_REQUIRED) {
    return (
      <p role="alert" className="text-sm text-destructive">
        {credentialErrors(error).form}{" "}
        <Link to="/sign-in" className="text-primary underline underline-offset-3">
          Sign in again
        </Link>{" "}
        to set your password.
      </p>
    );
  }
  return error ? (
    <p role="alert" className="text-sm text-destructive empty:hidden">
      {credentialErrors(error).form}
    </p>
  ) : (
    <output className="text-sm text-muted-foreground empty:hidden">
      {saved ? "Your password is saved, and you’re signed out everywhere else." : undefined}
    </output>
  );
}
