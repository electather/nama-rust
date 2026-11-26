import { SignInService } from "@/gen/nama/v1/sign_in_pb";
import { useSensitiveMutation } from "@/shared/hooks/use-sensitive-mutation";

/** Clears a User's Switch PIN, the caller's own without a `userId`, so switching to them needs none. */
export function useClearSwitchPin() {
  // Nothing to refetch: no User the Server lists says whether they have a Switch PIN.
  return useSensitiveMutation(SignInService.method.clearSwitchPin);
}
