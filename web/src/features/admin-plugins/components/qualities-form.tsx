import { create } from "@bufbuild/protobuf";
import { useState } from "react";

import { AddQuality } from "@/features/admin-plugins/components/add-quality";
import { QualityFields } from "@/features/admin-plugins/components/quality-fields";
import { useReplaceQualities } from "@/features/admin-plugins/data/use-replace-qualities";
import { useProfiles } from "@/features/admin-plugins/hooks/use-profiles";
import { keyed, qualitiesTyped, qualityErrors } from "@/features/admin-plugins/lib/qualities";
import { type Quality, QualitySchema } from "@/gen/nama/v1/plugin_pb";
import { FormEnd } from "@/shared/components/form-field/form-end";
import { onSubmitted } from "@/shared/lib/forms";
import type { PluginDetails } from "@/shared/lib/plugins";

/** The form that replaces the Plugin's Qualities with the rows typed, from the `saved` ones. */
export function QualitiesForm({ plugin, saved }: { plugin: PluginDetails; saved: Quality[] }) {
  const [rows, setRows] = useState(() => keyed(saved));
  const replace = useReplaceQualities((qualities) => {
    setRows(keyed(qualities));
  });
  const { profiles, unoffered, gone } = useProfiles(plugin, rows);
  const errors = { ...gone, ...(replace.error && qualityErrors(replace.error, rows, plugin)) };
  return (
    <form
      className="mt-4 flex max-w-md flex-col gap-5"
      onSubmit={onSubmitted((typed) => {
        replace.mutate({ id: plugin.summary.id, qualities: qualitiesTyped(typed, rows) });
      })}
    >
      <p role="alert" className="text-sm text-destructive empty:hidden">
        {unoffered}
      </p>
      {rows.map((row, index) => (
        <QualityFields
          key={row.key}
          rowKey={row.key}
          quality={row.quality}
          number={index + 1}
          profiles={profiles}
          errors={errors}
          onRemove={() => {
            setRows(rows.filter((each) => each.key !== row.key));
          }}
        />
      ))}
      <AddQuality
        onAdd={() => {
          const quality = create(QualitySchema, { isDefault: rows.length === 0 });
          setRows([...rows, ...keyed([quality])]);
        }}
      />
      <FormEnd
        action={replace.isPending ? "Saving…" : "Save Qualities"}
        variant="ghost"
        pending={replace.isPending}
        done={replace.isSuccess ? "Saved. New Requests get the default." : undefined}
        refusal={errors["form"]}
      />
    </form>
  );
}
