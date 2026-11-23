import { credentialErrors } from "@hub/features/account/lib/refusals";
import { Link } from "@tanstack/react-router";

/** That the Hub needs a sign-in in the last few minutes `toDo` something, and the way to one. */
export function SignInAgain({ error, toDo }: { error: Error | null; toDo: string }) {
  return (
    <p role="alert" className="text-sm text-destructive">
      {credentialErrors(error).form}{" "}
      <Link to="/sign-in" className="text-primary underline underline-offset-3">
        Sign in again
      </Link>{" "}
      {toDo}.
    </p>
  );
}
