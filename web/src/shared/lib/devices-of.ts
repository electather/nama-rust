import type { User } from "@/gen/nama/v1/sign_in_pb";

/** Whose Devices a `ListDevices` or `RevokeDevice` is about: the caller's own without a `userId`. */
export interface Whose {
  userId?: bigint;
}

/** `user`'s Devices, or the caller's own without a `user`. */
export function devicesOf(user?: User): Whose {
  return user === undefined ? {} : { userId: user.id };
}
