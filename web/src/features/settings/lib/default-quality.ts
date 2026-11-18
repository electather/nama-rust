import { ExtensionPoint, type Quality } from "@/gen/nama/v1/plugin_pb";

/** Each Fulfiller's field label: what it fetches. */
export const fulfillerLabels: Partial<Record<ExtensionPoint, string>> = {
  [ExtensionPoint.MOVIE_FULFILLER]: "Movies",
  [ExtensionPoint.SHOW_FULFILLER]: "Shows",
};

/** `quality`'s name, marked if it's the Fulfiller's default. */
export function marked(quality: Quality) {
  return quality.isDefault ? `${quality.name} (Default)` : quality.name;
}

/**
 * Why the User's default `quality` no longer applies, if `yours`, its Quality on the list, is gone,
 * and that `listDefault` does instead.
 */
export function removedFlag(
  quality: bigint | undefined,
  yours: Quality | undefined,
  listDefault: Quality | undefined,
) {
  const removed = quality !== undefined && yours === undefined;
  return removed
    ? `The Quality you picked is no longer offered, so your Requests get ${listDefault?.name}.`
    : undefined;
}
