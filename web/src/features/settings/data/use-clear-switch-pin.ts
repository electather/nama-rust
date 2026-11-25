import { SignInService } from "@/gen/nama/v1/sign_in_pb";
import { useSensitiveMutation } from "@/shared/hooks/use-sensitive-mutation";

/** Clears the signed-in User's own Switch PIN, so switching to them needs none. */
export function useClearSwitchPin() {
  return useSensitiveMutation(SignInService.method.clearSwitchPin);
}
