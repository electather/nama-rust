import type { Transport } from "@connectrpc/connect";
import { callUnaryMethod } from "@connectrpc/connect-query";

import { hubRefusalReason } from "@/features/auth/lib/hub-errors";
import { LinkService } from "@/gen/nama/hub/v1/link_pb";
import { RefusalReason } from "@/gen/nama/hub/v1/refusal_pb";

/**
 * The Server the sign-in `id` is for, and the Account's User there, empty for none; or `null` when
 * it expired or was used. Not cached: approving it uses it up.
 */
export async function lookUpServerSignIn(transport: Transport, id: string) {
  try {
    return await callUnaryMethod(transport, LinkService.method.getServerSignIn, { id });
  } catch (error) {
    if (hubRefusalReason(error) === RefusalReason.SERVER_SIGN_IN_NOT_FOUND) {
      return null;
    }
    throw error;
  }
}
