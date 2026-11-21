import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { refusal, serverMessage } from "@/shared/lib/forms";

/** Why the Server refused the `attempt` (such as "save the Job"), as a sentence. */
export function jobRefusal(error: unknown, attempt: string) {
  const told = { form: serverMessage(error) };
  const byReason = {
    [RefusalReason.SCHEDULE_INVALID]: told,
    [RefusalReason.JOB_NOT_FOUND]: told,
  };
  return refusal(error, attempt, {}, byReason).form;
}
