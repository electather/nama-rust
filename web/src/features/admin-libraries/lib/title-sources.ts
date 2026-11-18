import type { TitleSource } from "@/gen/nama/v1/library_pb";

/** Why a source in the order can't be used now: no listed source keeps it, or it isn't running. */
export function unusable(source: TitleSource | undefined) {
  if (source === undefined) {
    return "not available";
  }
  return source.running ? undefined : "not running";
}

/** What people know `source` by: its Plugin's name, or else its namespace. */
export function sourceName({ namespace, pluginName }: TitleSource) {
  return pluginName === "" ? namespace : pluginName;
}

const names = new Intl.ListFormat("en-GB", { type: "conjunction" });

/**
 * Why no source in `order` can search for titles, as a list of each in turn and whether it isn't
 * running or only adds artwork, or undefined when one can.
 */
export function whyNoSourceSearches(order: string[], listed: TitleSource[]) {
  const sources = order.map((namespace) => ({
    namespace,
    source: listed.find((each) => each.namespace === namespace),
  }));
  const oneSearches = sources.some(({ source }) => source?.running === true && source.searches);
  return oneSearches
    ? undefined
    : names.format(
        sources.map(({ namespace, source }) => {
          if (source === undefined) {
            return `${namespace} isn’t running`;
          }
          return source.running
            ? `${sourceName(source)} only adds artwork`
            : `${sourceName(source)} isn’t running`;
        }),
      );
}
