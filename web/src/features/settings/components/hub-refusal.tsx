import { SignInAgain } from "@/features/settings/components/sign-in-again";
import { needsRecentSignIn } from "@/features/settings/lib/account-refusals";

/**
 * Why the Hub refused a change to the Account with `error`, as `said`, or, when it wants a sign-in
 * in the last few minutes `toDo` it, the way to one. Nothing until it refuses.
 */
export function HubRefusal({
  error,
  said,
  toDo,
}: {
  error: Error | null;
  said: string | undefined;
  toDo: string;
}) {
  if (error === null) {
    return null;
  }
  return needsRecentSignIn(error) ? <SignInAgain error={error} toDo={toDo} /> : said;
}
