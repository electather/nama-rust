import type { Permission } from "@/gen/nama/v1/sign_in_pb";
import { useSignedInUser } from "@/shared/hooks/use-signed-in-user";
import { may } from "@/shared/lib/permissions";

/** Whether the signed-in User has `permission`, which is false until it's known. */
export function useMay(permission: Permission) {
  return may(useSignedInUser(), permission);
}
