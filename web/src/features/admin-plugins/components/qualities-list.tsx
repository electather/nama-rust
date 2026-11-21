import { useRef, useState } from "react";

import { QualitiesHead } from "@/features/admin-plugins/components/qualities-head";
import { QualityAdd } from "@/features/admin-plugins/components/quality-add";
import { QualityItem } from "@/features/admin-plugins/components/quality-item";
import { useReplaceQualities } from "@/features/admin-plugins/data/use-replace-qualities";
import { useProfiles } from "@/features/admin-plugins/hooks/use-profiles";
import {
  qualitiesDefaulting,
  qualitiesWith,
  qualitiesWithout,
  qualityRefusal,
} from "@/features/admin-plugins/lib/qualities";
import type { Quality } from "@/gen/nama/v1/plugin_pb";
import { type PluginDetails, pluginName } from "@/shared/lib/plugins";

/** What stands for the row a new Quality is typed into, among the ids of the saved ones. */
const NEW = "new";

const NOT_NOW = { opens: undefined, onAdd: undefined };

/**
 * A Fulfiller's `saved` Qualities as a list, with the button that adds one. One row at a time is
 * open: a saved Quality's fields under it, or the row a new one is typed into. Every change sends
 * the whole list at once, as the Server takes it. Nothing is added or edited while the service's
 * profiles can't be fetched, as there'd be none to pick from.
 */
export function QualitiesList({ plugin, saved }: { plugin: PluginDetails; saved: Quality[] }) {
  const [open, setOpen] = useState<string>();
  const add = useRef<HTMLButtonElement>(null);
  // The row a new Quality is typed into goes, so focus goes back to the button that opened it.
  const closeNew = () => {
    add.current?.focus();
    setOpen(undefined);
  };
  // The dialog that asks before removing says its own refusal; the rest are said under the list.
  const change = useReplaceQualities();
  const remove = useReplaceQualities();
  const { profiles, unoffered } = useProfiles(plugin);
  const { id } = plugin.summary;
  const { opens, onAdd } =
    unoffered === undefined
      ? {
          opens: setOpen,
          onAdd: () => {
            setOpen(NEW);
          },
        }
      : NOT_NOW;
  const items = saved.map((quality) => {
    const key = String(quality.id);
    return (
      <QualityItem
        key={key}
        plugin={plugin}
        quality={quality}
        profiles={profiles}
        open={open === key}
        onOpen={
          opens &&
          ((shown) => {
            opens(shown ? key : undefined);
          })
        }
        onSave={(typed) => {
          change.mutate({ id, qualities: qualitiesWith(saved, typed, quality) });
        }}
        onDefault={() => {
          change.mutate({ id, qualities: qualitiesDefaulting(saved, quality) });
        }}
        onRemove={async () => {
          await remove.mutateAsync({ id, qualities: qualitiesWithout(saved, quality) });
        }}
      />
    );
  });
  const adding = open === NEW && (
    <QualityAdd
      profiles={profiles}
      pending={change.isPending}
      onAdd={(typed) => {
        change.mutate({ id, qualities: qualitiesWith(saved, typed) }, { onSuccess: closeNew });
      }}
      onCancel={closeNew}
    />
  );
  return (
    <>
      <QualitiesHead ref={add} onAdd={onAdd} />
      <p className="mt-1 max-w-prose text-sm text-pretty text-muted-foreground">
        Each is one of {pluginName(plugin.summary)}’s quality profiles under a name you choose. A
        new Request gets the default.
      </p>
      <p role="alert" className="mt-3 text-sm text-pretty text-destructive empty:hidden">
        {unoffered}
      </p>
      <ul aria-labelledby="qualities" className="peer mt-1 -mb-3.5 flex flex-col empty:hidden">
        {items}
        {adding}
      </ul>
      <p className="mt-3 hidden text-sm text-pretty text-muted-foreground peer-empty:block">
        None yet, so Requests use the Quality profile setting.
      </p>
      {change.isError && (
        <p role="alert" className="mt-4 text-sm text-pretty text-destructive">
          {qualityRefusal(change.error, plugin)}
        </p>
      )}
    </>
  );
}
