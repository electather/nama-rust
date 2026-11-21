import { useSuspenseQuery } from "@connectrpc/connect-query";

import { UserService } from "@/gen/nama/v1/user_pb";

/** The Devices the signed-in User is signed in on. */
export function useYourDevices() {
  return useSuspenseQuery(UserService.method.listDevices, {}).data.devices;
}
