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
      [RefusalReason.RECOGNISERS_INVALID]: orderRefused(error, "recogniser", "recognisers"),
      [RefusalReason.SEGMENT_SOURCES_INVALID]: orderRefused(
        error,
        "segment_source",
        "segmentSources",
      ),
    },
  );
}

/**
 * Where a Plugin order refusal shows: on its `key` field when it names `<field>_ids` (a create) or
 * `<field>_order` (an update), else for the whole form.
 */
function orderRefused(
  error: unknown,
  field: string,
  key: "recognisers" | "segmentSources",
): FormErrors {
  const onTheField = new RegExp(`^${field}_(?:ids|order)\\b`, "u").test(refusalOf(error).field);
  return onTheField ? { [key]: serverMessage(error) } : { form: serverMessage(error) };
}
