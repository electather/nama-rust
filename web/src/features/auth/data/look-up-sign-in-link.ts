import type { Transport } from "@connectrpc/connect";
import { callUnaryMethod } from "@connectrpc/connect-query";

import { hubRefusalReason } from "@/features/auth/lib/hub-errors";
import { AccountService } from "@/gen/nama/hub/v1/account_pb";
import { RefusalReason } from "@/gen/nama/hub/v1/refusal_pb";

/**
 * The Account the Sign-in Link `token` signs in, without using it up, or `null` when it expired,
 * was used or never existed. Not cached: once used, asking again says it expired.
 */
export async function lookUpSignInLink(transport: Transport, token: string) {
  try {
    return await callUnaryMethod(transport, AccountService.method.getSignInLink, { token });
  } catch (error) {
    if (hubRefusalReason(error) === RefusalReason.SIGN_IN_LINK_EXPIRED) {
      return null;
    }
    throw error;
  }
}
