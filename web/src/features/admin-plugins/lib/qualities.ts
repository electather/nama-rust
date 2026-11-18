import { type Choice, ExtensionPoint, type Quality, SettingKind } from "@/gen/nama/v1/plugin_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { type PluginDetails, pluginName } from "@/lib/plugins";
import { formText, refusal, refusalOf, serverMessage } from "@/shared/lib/forms";

/** The choice setting whose choices a Fulfiller's Qualities offer (`qualities.rs` on the Server). */
export const qualityProfile = "quality_profile";

/** Whether `plugin` takes Qualities: a Fulfiller with the `quality_profile` choice setting. */
export function takesQualities(plugin: PluginDetails) {
  return (
    plugin.summary.extensionPoints.some(
      (point) =>
        point === ExtensionPoint.MOVIE_FULFILLER || point === ExtensionPoint.SHOW_FULFILLER,
    ) &&
    plugin.settings.some(({ key, kind }) => key === qualityProfile && kind === SettingKind.CHOICE)
  );
}

/** A Quality being edited, keyed so its inputs keep their names as others are removed. */
export interface Row {
  key: number;
  quality: Quality;
}

const keys = { last: 0 };

/** `qualities` as rows, each with a key no other row ever had. */
export function keyed(qualities: Quality[]): Row[] {
  return qualities.map((quality) => {
    keys.last += 1;
    return { key: keys.last, quality };
  });
}

function notOffered(plugin: PluginDetails) {
  return `${pluginName(plugin.summary)} doesn’t offer this profile now.`;
}

/** A flag under each of `rows` whose saved profile `plugin` doesn't offer now, as `profiles` says. */
export function profilesGone(rows: Row[], profiles: Choice[] | undefined, plugin: PluginDetails) {
  if (profiles === undefined) {
    return {};
  }
  const gone = rows.filter(
    ({ quality: { profileId } }) =>
      profileId !== "" && !profiles.some(({ value }) => value === profileId),
  );
  return Object.fromEntries(gone.map(({ key }) => [`profile-${key}`, notOffered(plugin)]));
}

/** The Qualities typed in `typed`, one per row, in order. */
export function qualitiesTyped(typed: FormData, rows: Row[]) {
  return rows.map(({ key, quality }) => ({
    id: quality.id,
    profileId: formText(typed, `profile-${key}`),
    name: formText(typed, `name-${key}`),
    description: formText(typed, `description-${key}`),
    isDefault: formText(typed, "default") === String(key),
  }));
}

/** The row and input a Refusal's `field`, such as `qualities[1].name`, names, if it names one. */
function refusedInput(field: string, rows: Row[]) {
  const [, index, part] =
    /^qualities\[(?<index>\d+)\]\.(?<part>profile_id|name)$/u.exec(field) ?? [];
  const row = rows[Number(index)];
  return row && ([row, part === "name" ? "name" : "profile"] as const);
}

/** Why the Server refused the Qualities as a whole. */
function formRefused(error: unknown) {
  return refusal(
    error,
    "save the Qualities",
    {},
    {
      [RefusalReason.DEFAULT_QUALITY_REQUIRED]: { form: "Mark one Quality as the default." },
      // The Plugin's own words.
      [RefusalReason.SETTINGS_CHECK_FAILED]: { form: serverMessage(error) },
    },
  ).form;
}

/** Why the Server refused a Quality's name or profile, by its Refusal's reason. */
function inputRefused(
  plugin: PluginDetails,
): Record<"name" | "profile", Partial<Record<RefusalReason, string>>> {
  return {
    name: { [RefusalReason.QUALITY_REPEATED]: "Another Quality has this name." },
    profile: {
      [RefusalReason.QUALITY_REPEATED]: "Another Quality offers this profile.",
      [RefusalReason.CHOICE_NOT_OFFERED]: notOffered(plugin),
    },
  };
}

/** Why the Server refused the Qualities: next to the input its Refusal names, or for the form. */
export function qualityErrors(
  error: unknown,
  rows: Row[],
  plugin: PluginDetails,
): Record<string, string | undefined> {
  const { reason, field } = refusalOf(error);
  const named = refusedInput(field, rows);
  if (named === undefined) {
    return { form: formRefused(error) };
  }
  const [row, input] = named;
  return { [`${input}-${row.key}`]: inputRefused(plugin)[input][reason] ?? serverMessage(error) };
}
