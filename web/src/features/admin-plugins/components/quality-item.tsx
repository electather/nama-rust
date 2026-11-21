import { useRef } from "react";

import { QualityEdit } from "@/features/admin-plugins/components/quality-edit";
import { QualityMenu } from "@/features/admin-plugins/components/quality-menu";
import { profileFlag, qualityMeta, qualityRefusal } from "@/features/admin-plugins/lib/qualities";
import type { Choice, Quality } from "@/gen/nama/v1/plugin_pb";
import { ConfirmDialog } from "@/shared/components/confirm-dialog/confirm-dialog";
import type { Opener } from "@/shared/lib/opener";
import type { PluginDetails } from "@/shared/lib/plugins";

/**
 * One Quality on a Fulfiller's list: its name, whether it's the default, its profile and
 * description, a flag if the service doesn't offer its profile now, and its "…". While `open` its
 * fields show under it. Each change saves at once. Removing it asks first, as Requesters lose the
 * choice, and rejects if the Server refuses.
 */
export function QualityItem({
  plugin,
  quality,
  profiles,
  open,
  onOpen,
  onSave,
  onDefault,
  onRemove,
}: {
  plugin: PluginDetails;
  quality: Quality;
  profiles: Choice[] | undefined;
  open: boolean;
  /** Opens its fields, or closes them. Not given while there are no profiles to pick from. */
  onOpen: ((open: boolean) => void) | undefined;
  onSave: (typed: FormData) => void;
  onDefault: () => void;
  onRemove: () => Promise<unknown>;
}) {
  const row = useRef<HTMLLIElement>(null);
  const removing = useRef<Opener>(null);
  const isDefault = quality.isDefault && (
    <span className="rounded-full border px-1.5 text-xs font-normal text-muted-foreground">
      Default
    </span>
  );
  const words = (
    <div className="flex min-w-0 flex-1 flex-col gap-0.5">
      <span className="flex flex-wrap items-center gap-x-2 gap-y-1 text-sm font-semibold">
        {quality.name}
        {isDefault}
      </span>
      <span className="text-xs text-pretty text-muted-foreground">
        {qualityMeta(quality, profiles)}
      </span>
      <span className="text-xs text-pretty text-destructive empty:hidden">
        {profileFlag(quality, profiles, plugin)}
      </span>
    </div>
  );
  const head = (
    <div className="flex items-center gap-x-3.5">
      {words}
      <QualityMenu
        quality={quality}
        onEdit={
          onOpen &&
          (() => {
            onOpen(true);
          })
        }
        onDefault={onDefault}
        onRemove={() => removing.current?.open()}
      />
    </div>
  );
  return (
    <li ref={row} className="py-3.5 not-first:border-t">
      {head}
      {open && (
        <QualityEdit
          quality={quality}
          profiles={profiles}
          onSave={onSave}
          onDone={() => {
            // Done goes with the fields, so focus goes back to the row's "…".
            row.current?.querySelector("button")?.focus();
            onOpen?.(false);
          }}
        />
      )}
      <ConfirmDialog
        ref={removing}
        trigger={false}
        name={`Remove ${quality.name}`}
        action="Remove"
        title={`Remove ${quality.name}?`}
        description="Requesters can no longer pick it."
        onConfirm={onRemove}
        refusal={(error) => qualityRefusal(error, plugin)}
      />
    </li>
  );
}
