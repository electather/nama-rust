import type { Transport } from "@connectrpc/connect";
import { callUnaryMethod } from "@connectrpc/connect-query";

import { LinkService } from "@/gen/nama/hub/v1/link_pb";

/**
 * The Server the sign-in `id` is for, and the Account's User there, empty for none. Not cached:
 * approving it uses it up.
 */
export async function lookUpServerSignIn(transport: Transport, id: string) {
  const signIn = await callUnaryMethod(transport, LinkService.method.getServerSignIn, { id });
  return signIn;
}
