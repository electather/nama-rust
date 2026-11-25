import { SignInService } from "@/gen/nama/v1/sign_in_pb";
import { useSensitiveMutation } from "@/shared/hooks/use-sensitive-mutation";

/** Sets or changes the signed-in User's own Switch PIN. */
export function useSetSwitchPin() {
  return useSensitiveMutation(SignInService.method.setSwitchPin);
}
