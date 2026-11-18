import type { ConnectError } from "@connectrpc/connect";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { refusal, serverMessage } from "@/shared/lib/forms";

/** Why the Server refused the Server URL, under its field, or `{}` while it hasn't. */
export function serverUrlErrors(error: ConnectError | null) {
  if (error === null) {
    return {};
  }
  return refusal(
    error,
    "save the Server URL",
    {},
    { [RefusalReason.NOT_A_URL]: { serverUrl: serverMessage(error) } },
  );
}
