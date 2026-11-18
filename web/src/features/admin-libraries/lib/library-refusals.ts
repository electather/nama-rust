import { Code } from "@connectrpc/connect";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import {
  type FormErrors,
  refusalOf,
  refusedFieldErrors,
  refusal,
  serverMessage,
} from "@/shared/lib/forms";

/** Why the Server refused to `attempt` (such as "create the Library"), next to its field. */
export function libraryErrors(error: unknown, attempt: string): FormErrors {
  return refusal(
    error,
    attempt,
    {
      [Code.InvalidArgument]: refusedFieldErrors(error, ["name", "folders"]),
      [Code.AlreadyExists]: { folders: serverMessage(error) },
      [Code.NotFound]: { form: "This Library isn’t on the Server any more." },
    },
    {
      [RefusalReason.LIBRARY_NAME_TAKEN]: { name: serverMessage(error) },
      [RefusalReason.TITLE_SOURCES_INVALID]: { titleSources: serverMessage(error) },
      [RefusalReason.RECOGNISERS_INVALID]: recognisersRefused(error),
    },
  );
}

/**
 * Where a Recognisers refusal shows: on the Recognisers field when it names `recogniser_ids` (a
 * create) or `recogniser_order` (an update), else for the whole form.
 */
function recognisersRefused(error: unknown): FormErrors {
  const onTheField = /^recogniser_(?:ids|order)\b/u.test(refusalOf(error).field);
  return onTheField ? { recognisers: serverMessage(error) } : { form: serverMessage(error) };
}
