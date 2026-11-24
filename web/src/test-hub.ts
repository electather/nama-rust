import { create } from "@bufbuild/protobuf";
import { type Code, ConnectError } from "@connectrpc/connect";

import { AccountSchema } from "@/gen/nama/hub/v1/account_pb";
import { type RefusalReason, RefusalSchema } from "@/gen/nama/hub/v1/refusal_pb";

/** The Account the fake Hubs know, whose password is "correct horse". */
export const ada = create(AccountSchema, { id: "0192-ada", email: "ada@example.com" });

/** A Connect error as the Hub refuses with one. */
export function hubRefused(code: Code, reason: RefusalReason, params: Record<string, string> = {}) {
  return new ConnectError("refused", code, undefined, [
    { desc: RefusalSchema, value: create(RefusalSchema, { reason, params }) },
  ]);
}
