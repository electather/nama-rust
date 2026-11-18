import { useEffect } from "react";

import { usePollSignInCode } from "@/features/auth/data/use-poll-sign-in-code";
import { isUndecided, statusOf } from "@/features/auth/lib/sign-in-code-status";
import { SignInCodeStatus } from "@/gen/nama/v1/sign_in_pb";

// ponytail: fixed-interval polling, which is what the API offers.
const pollEvery = 3000;

/**
 * Polls the sign-in `handle` stands for until it's approved, then calls `onSignedIn`. Returns
 * whether it expired instead.
 */
export function usePollUntilApproved(handle: string, onSignedIn: () => Promise<void>) {
  const { mutate, isPending, data, error } = usePollSignInCode(onSignedIn);

  // Runs again after every poll, since each brings a new `data` or `error`. `isPending` alone
  // wouldn't do: a quick answer can arrive before React renders the pending state.
  useEffect(() => {
    const poll = () => {
      mutate({ handle });
    };
    const pollAgain = !isPending && isUndecided(statusOf(data, error));
    const timer = pollAgain ? setTimeout(poll, pollEvery) : undefined;
    return () => {
      clearTimeout(timer);
    };
  }, [isPending, data, error, handle, mutate]);
  return statusOf(data, error) === SignInCodeStatus.EXPIRED;
}
