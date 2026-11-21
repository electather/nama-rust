import { ExtensionPoint, type PluginSummary } from "@/gen/nama/v1/plugin_pb";

/** One kind of Plugin a Library asks in an order an Admin sets, as the order field shows it. */
export interface PluginOrderKind {
  /** The field's legend, which also names its list. */
  legend: string;
  /** The form value sent once per Plugin id. */
  name: string;
  point: ExtensionPoint;
  /** What the field says it does, or, while `noneInstalled`, what happens without any. */
  hint: (noneInstalled: boolean) => string;
}

/** The Plugins a Library asks what a file is. */
export const RECOGNISERS: PluginOrderKind = {
  legend: "Recognisers",
  name: "recognisers",
  point: ExtensionPoint.RECOGNISER,
  hint: (noneInstalled) =>
    noneInstalled
      ? "No Recogniser is installed, so the Server’s own Matching reads every file."
      : "Each file goes to these Recognisers in order, and the first that can tell says what it is. The Server’s own Matching reads the rest.",
};

/** The Plugins a Library asks where its Versions' Intro, Recap and Credits are (ADR 0075). */
export const SEGMENT_SOURCES: PluginOrderKind = {
  legend: "Segment sources",
  name: "segmentSources",
  point: ExtensionPoint.SEGMENT_SOURCE,
  hint: (noneInstalled) =>
    noneInstalled
      ? "No Segment source is installed, so Segments come only from Chapter names."
      : "After Chapter names, these Segment sources are asked in order where the Intro, Recap and Credits are, and the first answer for each wins.",
};

/** The Plugins of `summaries` that serve `point`. */
export function pluginsIn(point: ExtensionPoint, summaries: PluginSummary[] = []) {
  return summaries.filter((plugin) => plugin.extensionPoints.includes(point));
}

/** What admins know the Plugin `pluginId` by: the name of its `plugin`, or else its id. */
export function labelOf(pluginId: string, plugin: PluginSummary | undefined) {
  return plugin === undefined || plugin.name === "" ? pluginId : plugin.name;
}

/**
 * A fieldset ref that runs `reset` when its form is reset, as CreateLibrary's is after a create,
 * which leaves alone the hidden inputs state controls. React has no reset handler for a fieldset.
 */
export function resetWith(reset: () => void) {
  return (fieldset: HTMLFieldSetElement | null) => {
    const form = fieldset?.form;
    form?.addEventListener("reset", reset);
    return () => form?.removeEventListener("reset", reset);
  };
}

/** What each of a row's buttons does to `order`, or `undefined` where it can't. */
export interface Moves {
  up: (() => void) | undefined;
  down: (() => void) | undefined;
  remove: () => void;
}

/** The changes to `order` the row at `index` offers, each handed to `setOrder`. */
export function moves(order: string[], index: number, setOrder: (order: string[]) => void): Moves {
  const swapWith = (other: number) => () => {
    const next = [...order];
    [next[index], next[other]] = [next[other] ?? "", next[index] ?? ""];
    setOrder(next);
  };
  return {
    up: index > 0 ? swapWith(index - 1) : undefined,
    down: index < order.length - 1 ? swapWith(index + 1) : undefined,
    remove: () => {
      setOrder(order.filter((_, each) => each !== index));
    },
  };
}
