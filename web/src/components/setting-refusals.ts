import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { refusal, refusalOf, serverMessage } from "@/lib/forms";

/** Why the Server refused the `attempt`: next to the setting its Refusal names, or for the form. */
export function refusedSettings(
  error: unknown,
  attempt: string,
): Record<string, string | undefined> {
  const { reason, field, params } = refusalOf(error);
  const key = field.replace(/^settings\./u, "");
  if (key === field) {
    const { form } = refusal(
      error,
      attempt,
      {},
      {
        [RefusalReason.PLUGIN_NOT_FOUND]: { form: "This Plugin isn’t on the Server any more." },
        // The Plugin's own words.
        [RefusalReason.SETTINGS_CHECK_FAILED]: { form: serverMessage(error) },
      },
    );
    return { form };
  }
  // A failed check is in the Plugin's own words.
  if (reason === RefusalReason.SETTINGS_CHECK_FAILED) return { [key]: serverMessage(error) };
  return { [key]: settingRefusal(reason, params) };
}

/** Why the Server refused a setting: an out-of-range one says its bounds, the rest go by reason. */
function settingRefusal(reason: RefusalReason, params: Record<string, string | undefined>) {
  if (reason === RefusalReason.SETTING_OUT_OF_RANGE) return outOfRange(params);
  return settingRefused[reason] ?? "This setting can’t take that value.";
}

/** The bounds a number must keep to, from its Refusal's `min` and `max`, at least one of which the Server sends. */
function outOfRange({ min, max }: Record<string, string | undefined>) {
  if (max === undefined) return `This must be at least ${min}.`;
  return min === undefined ? `This must be at most ${max}.` : `This must be from ${min} to ${max}.`;
}

/** Why the Server refused a setting, by its Refusal's reason. */
const settingRefused: Partial<Record<RefusalReason, string>> = {
  [RefusalReason.FIELD_REQUIRED]: "This setting is required.",
  [RefusalReason.CHOICE_NOT_OFFERED]:
    "The Plugin doesn’t offer this choice now. Test the settings and pick again.",
  [RefusalReason.LIBRARY_NOT_FOUND]: "A Library picked here isn’t on the Server any more.",
};
