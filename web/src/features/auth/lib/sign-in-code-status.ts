import { Code } from "@connectrpc/connect";

import { type PollSignInCodeResponse, SignInCodeStatus } from "@/gen/nama/v1/sign_in_pb";
import { isCode } from "@/shared/lib/connect-errors";

/** Where a sign-in has got to, going by its last poll: none before the first, or after a failed one. */
export function statusOf(data: PollSignInCodeResponse | undefined, error: unknown) {
  // A Server restart forgets the handle, so `not_found` means the code is gone too.
  return isCode(error, Code.NotFound) ? SignInCodeStatus.EXPIRED : data?.status;
}

// Without a status, such as after a poll failed while the Server restarted, it polls again.
/** Whether a sign-in with this status should be polled again. */
export function isUndecided(status: SignInCodeStatus | undefined) {
  return status !== SignInCodeStatus.EXPIRED && status !== SignInCodeStatus.SIGNED_IN;
}
