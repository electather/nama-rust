import type { ConnectError } from "@connectrpc/connect";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { formText, refusal, serverMessage } from "@/shared/lib/forms";

/** The image settings the form holds `typed`. */
export function typedSettings(typed: FormData) {
  const mib = formText(typed, "resizedCacheMib");
  return {
    resizeImages: typed.has("resizeImages"),
    // Empty leaves it unset, for the Server's default.
    ...(mib !== "" && { resizedCacheMib: Number(mib) }),
  };
}

/** Why the Server refused the image settings, under their field, or `{}` while it hasn't. */
export function imageSettingsErrors(error: ConnectError | null) {
  if (error === null) {
    return {};
  }
  return refusal(
    error,
    "save the image settings",
    {},
    { [RefusalReason.FIELD_REQUIRED]: { resizedCacheMib: serverMessage(error) } },
  );
}
