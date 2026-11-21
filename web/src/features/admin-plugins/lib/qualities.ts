import { type Choice, ExtensionPoint, type Quality, SettingKind } from "@/gen/nama/v1/plugin_pb";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { formText, refusal, refusalOf, serverMessage } from "@/shared/lib/forms";
import { type PluginDetails, pluginName } from "@/shared/lib/plugins";

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

function notOffered(plugin: PluginDetails) {
  return `${pluginName(plugin.summary)} doesn’t offer this profile now.`;
}

/**
 * The flag under a Quality whose profile `plugin`'s service doesn't offer now, as `profiles` says.
 * None while they aren't known: not fetched yet, or the check failed.
 */
export function profileFlag(
  quality: Quality,
  profiles: Choice[] | undefined,
  plugin: PluginDetails,
) {
  const offered = profiles?.some(({ value }) => value === quality.profileId) ?? true;
  return offered ? undefined : notOffered(plugin);
}

/** The line under a Quality's name: its profile as the service names it, else its id, then its description. */
export function qualityMeta(quality: Quality, profiles: Choice[] = []) {
  const named = profiles.find(({ value }) => value === quality.profileId)?.label;
  return [named ?? quality.profileId, quality.description]
    .filter((part) => part !== "")
    .join(" · ");
}

/** A Quality as a list to save holds it. */
function listed({ id, profileId, name, description, isDefault }: Quality) {
  return { id, profileId, name, description, isDefault };
}

/**
 * `saved` with the Quality `typed` into its dialog in place of `edited`, or at the end without one,
 * where the first Quality of a list is its default.
 */
export function qualitiesWith(saved: Quality[], typed: FormData, edited?: Quality) {
  const fields = {
    profileId: formText(typed, "profile"),
    name: formText(typed, "name"),
    description: formText(typed, "description"),
  };
  if (edited === undefined) {
    const kept = saved.map((quality) => listed(quality));
    return [...kept, { id: 0n, ...fields, isDefault: saved.length === 0 }];
  }
  return saved.map((quality) =>
    quality.id === edited.id ? { ...listed(quality), ...fields } : listed(quality),
  );
}

/** `saved` without `removed`. */
export function qualitiesWithout(saved: Quality[], removed: Quality) {
  return saved.filter(({ id }) => id !== removed.id).map((quality) => listed(quality));
}

/** `saved` with `chosen` its only default. */
export function qualitiesDefaulting(saved: Quality[], chosen: Quality) {
  return saved.map((quality) => ({ ...listed(quality), isDefault: quality.id === chosen.id }));
}

/** Why the Server refused the Qualities as a whole. */
function formRefused(error: unknown) {
  return refusal(
    error,
    "save the Qualities",
    {},
    {
      // Only removing the default while others are left sends a list without one.
      [RefusalReason.DEFAULT_QUALITY_REQUIRED]: { form: "Make another Quality the default first." },
      // The Plugin's own words.
      [RefusalReason.SETTINGS_CHECK_FAILED]: { form: serverMessage(error) },
    },
  ).form;
}

/**
 * Why the Server refused a Quality's name or profile, by the part of it the Refusal names and the
 * Refusal's reason.
 */
function inputRefused(
  plugin: PluginDetails,
): Record<string, Partial<Record<RefusalReason, string>> | undefined> {
  return {
    name: { [RefusalReason.QUALITY_REPEATED]: "Another Quality has this name." },
    profile_id: {
      [RefusalReason.QUALITY_REPEATED]: "Another Quality offers this profile.",
      [RefusalReason.CHOICE_NOT_OFFERED]: notOffered(plugin),
    },
  };
}

/** The part of a Quality a Refusal's `field`, such as `qualities[1].name`, names, or `""`. */
function refusedPart(field: string) {
  return /^qualities\[\d+\]\.(?<part>\w+)$/u.exec(field)?.groups?.["part"] ?? "";
}

/**
 * Why the Server refused a change to the Qualities, as a sentence: what its Refusal says of the
 * name or profile it names, or of the list.
 */
export function qualityRefusal(error: unknown, plugin: PluginDetails) {
  const { reason, field } = refusalOf(error);
  const said = inputRefused(plugin)[refusedPart(field)];
  return said === undefined ? formRefused(error) : (said[reason] ?? serverMessage(error));
}
