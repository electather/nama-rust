import { useQuery } from "@connectrpc/connect-query";
import { ArrowDown, ArrowUp, X } from "lucide-react";
import { useId, useState } from "react";

import { Button } from "@/components/ui/button";
import { ExtensionPoint, type PluginSummary, PluginService } from "@/gen/nama/v1/plugin_pb";

const none: string[] = [];

/**
 * The Library's Recognisers in the order it asks them, which an Admin picks from every Recogniser
 * Plugin, enabled or not, and reorders. Sent as one `recognisers` form value per Plugin id. None
 * leaves the Server's own Matching alone.
 */
export function RecognisersField({
  initial = none,
  error,
}: {
  initial?: string[] | undefined;
  error?: string | undefined;
}) {
  const id = useId();
  const [order, setOrder] = useState(initial);
  // Only `Manage settings` lists Plugins. Without the list, the stored ids show as they are.
  const { data } = useQuery(PluginService.method.listPlugins, {});
  const listed = data !== undefined;
  const recognisers = recognisersIn(data?.summaries);

  return (
    <fieldset
      className="flex flex-col gap-2"
      aria-describedby={`${id}-hint ${id}-error`}
      ref={resetWith(() => {
        setOrder(initial);
      })}
    >
      <legend className="mb-2 text-sm font-medium">Recognisers</legend>
      <RecogniserList order={order} setOrder={setOrder} recognisers={recognisers} listed={listed} />
      <AddRecognisers
        recognisers={recognisers.filter((plugin) => !order.includes(plugin.id))}
        add={(pluginId) => {
          setOrder([...order, pluginId]);
        }}
      />
      <p id={`${id}-hint`} className="text-sm text-muted-foreground">
        {hint(listed && recognisers.length === 0 && order.length === 0)}
      </p>
      {/* Always there, so screen readers announce an error as it appears. */}
      <p id={`${id}-error`} role="alert" className="text-sm text-destructive empty:hidden">
        {error}
      </p>
    </fieldset>
  );
}

/**
 * `order` as a list of rows, each with its Plugin among `recognisers`, once the Plugins are
 * `listed`. Nothing while it's empty.
 */
function RecogniserList({
  order,
  setOrder,
  recognisers,
  listed,
}: {
  order: string[];
  setOrder: (order: string[]) => void;
  recognisers: PluginSummary[];
  listed: boolean;
}) {
  if (order.length === 0) return null;
  return (
    <ol aria-label="Recognisers" className="flex flex-col gap-1">
      {order.map((pluginId, index) => (
        <RecogniserRow
          key={pluginId}
          pluginId={pluginId}
          plugin={recognisers.find((each) => each.id === pluginId)}
          listed={listed}
          moves={moves(order, index, setOrder)}
        />
      ))}
    </ol>
  );
}

/**
 * A fieldset ref that runs `reset` when its form is reset, as CreateLibrary's is after a create,
 * which leaves alone the hidden inputs state controls. React has no reset handler for a fieldset.
 */
function resetWith(reset: () => void) {
  return (fieldset: HTMLFieldSetElement | null) => {
    const form = fieldset?.form;
    form?.addEventListener("reset", reset);
    return () => form?.removeEventListener("reset", reset);
  };
}

/** A button per Recogniser in `recognisers`, which `add`s it at the end of the order. */
function AddRecognisers({
  recognisers,
  add,
}: {
  recognisers: PluginSummary[];
  add: (pluginId: string) => void;
}) {
  return (
    <div className="-ms-4 flex flex-wrap gap-1">
      {recognisers.map((plugin) => (
        <Button
          key={plugin.id}
          type="button"
          variant="ghost"
          onClick={() => {
            add(plugin.id);
          }}
        >
          Add {labelOf(plugin.id, plugin)}
          {!plugin.enabled && <NotEnabled />}
        </Button>
      ))}
    </div>
  );
}

/** The Plugins of `summaries` that are Recognisers. */
function recognisersIn(summaries: PluginSummary[] = []) {
  return summaries.filter((plugin) => plugin.extensionPoints.includes(ExtensionPoint.RECOGNISER));
}

/** What the field says it does, which, while `noneInstalled`, is leave Matching to the Server. */
function hint(noneInstalled: boolean) {
  return noneInstalled
    ? "No Recogniser is installed, so the Server’s own Matching reads every file."
    : "Each file goes to these Recognisers in order, and the first that can tell says what it is. The Server’s own Matching reads the rest.";
}

/** What admins know the Plugin `pluginId` by: the name of its `plugin`, or else its id. */
function labelOf(pluginId: string, plugin: PluginSummary | undefined) {
  return plugin === undefined || plugin.name === "" ? pluginId : plugin.name;
}

/**
 * The note on a row for `plugin`: that it's off, which the Library skips until it's enabled, or,
 * once the Plugins are `listed`, that there's no such Plugin. Until the list arrives, no stored
 * Recogniser is known to be missing.
 */
function RowNote({ plugin, listed }: { plugin: PluginSummary | undefined; listed: boolean }) {
  if (plugin === undefined) {
    return listed ? <span className="text-muted-foreground"> · not available</span> : null;
  }
  return plugin.enabled ? null : <NotEnabled />;
}

/** The note beside a Recogniser whose Plugin is off. */
function NotEnabled() {
  return <span className="text-muted-foreground"> · not enabled</span>;
}

/** What each of a row's buttons does to `order`, or `undefined` where it can't. */
interface Moves {
  up: (() => void) | undefined;
  down: (() => void) | undefined;
  remove: () => void;
}

/** The changes to `order` the row at `index` offers, each handed to `setOrder`. */
function moves(order: string[], index: number, setOrder: (order: string[]) => void): Moves {
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

/** One Recogniser in the order, with the buttons that move it or take it out. */
function RecogniserRow({
  pluginId,
  plugin,
  listed,
  moves: { up, down, remove },
}: {
  pluginId: string;
  plugin: PluginSummary | undefined;
  listed: boolean;
  moves: Moves;
}) {
  const label = labelOf(pluginId, plugin);
  const buttons = [
    { name: `Move ${label} up`, icon: ArrowUp, onClick: up },
    { name: `Move ${label} down`, icon: ArrowDown, onClick: down },
    { name: `Remove ${label}`, icon: X, onClick: remove },
  ];
  return (
    <li className="flex items-center gap-1 text-sm">
      <input type="hidden" name="recognisers" value={pluginId} />
      <span className="me-auto">
        {label}
        <RowNote plugin={plugin} listed={listed} />
      </span>
      {buttons.map(({ name, icon: Icon, onClick }) => (
        <Button
          key={name}
          type="button"
          variant="ghost"
          size="icon"
          aria-label={name}
          disabled={onClick === undefined}
          onClick={onClick}
        >
          <Icon aria-hidden />
        </Button>
      ))}
    </li>
  );
}
