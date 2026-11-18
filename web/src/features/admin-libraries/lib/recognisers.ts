import { ExtensionPoint, type PluginSummary } from "@/gen/nama/v1/plugin_pb";

/** The Plugins of `summaries` that are Recognisers. */
export function recognisersIn(summaries: PluginSummary[] = []) {
  return summaries.filter((plugin) => plugin.extensionPoints.includes(ExtensionPoint.RECOGNISER));
}

/** What the field says it does, which, while `noneInstalled`, is leave Matching to the Server. */
export function recognisersHint(noneInstalled: boolean) {
  return noneInstalled
    ? "No Recogniser is installed, so the Server’s own Matching reads every file."
    : "Each file goes to these Recognisers in order, and the first that can tell says what it is. The Server’s own Matching reads the rest.";
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
