import { type Refused, refusedWords } from "@/features/settings/lib/refused-words";
import { serverLabels } from "@/features/settings/lib/server-values";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { serverMessage } from "@/shared/lib/forms";

/**
 * What the Server pane says of the change the Server `refused`: a Server URL it won't take under
 * that field, anything else in the pane's toast.
 */
export function serverRefusal(refused: Refused | undefined) {
  const byReason = refused && {
    [RefusalReason.NOT_A_URL]: { serverUrl: serverMessage(refused.failure) },
  };
  return refusedWords(refused, serverLabels, byReason);
}
