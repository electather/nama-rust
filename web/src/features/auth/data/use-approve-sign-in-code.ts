import { SignInService } from "@/gen/nama/v1/sign_in_pb";
import { useSensitiveMutation } from "@/shared/hooks/use-sensitive-mutation";

/** Signs in the Device a Sign-in Code belongs to, once the User confirms it's them. */
export function useApproveSignInCode() {
  return useSensitiveMutation(SignInService.method.approveSignInCode);
}
