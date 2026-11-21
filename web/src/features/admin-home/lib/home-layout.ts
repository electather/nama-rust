import type { ConnectError } from "@connectrpc/connect";

import { refusal } from "@/shared/lib/forms";

/** Why the Server refused to save the layout, or `undefined` while it hasn't. */
export function saveRefused(error: ConnectError | null) {
  return error === null ? undefined : refusal(error, "save the Home layout", {}).form;
}
