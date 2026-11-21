import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { refusal } from "@/shared/lib/forms";

/** Why Run now didn't start the Job, by its Refusal (`web/AGENTS.md`). */
export function runRefused(error: unknown) {
  return refusal(
    error,
    "start it",
    {},
    {
      [RefusalReason.JOB_RUNNING]: { form: "It’s running now. Run it again once that Run ends." },
      [RefusalReason.JOB_NOT_FOUND]: { form: "Its Plugin is disabled or no longer has it." },
    },
  ).form;
}
