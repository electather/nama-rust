import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { serverMessage } from "@/shared/lib/forms";

/** Where a Job's settings say the refusal `failure`, by its reason: under its schedule, or in its toast. */
export function jobRefusalFields(failure: unknown) {
  const message = serverMessage(failure);
  return {
    [RefusalReason.SCHEDULE_INVALID]: { schedule: message },
    [RefusalReason.JOB_NOT_FOUND]: { form: message },
  };
}
