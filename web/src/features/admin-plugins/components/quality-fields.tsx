import { ProfileSelect } from "@/features/admin-plugins/components/profile-select";
import { QualityEnd } from "@/features/admin-plugins/components/quality-end";
import type { Choice, Quality } from "@/gen/nama/v1/plugin_pb";
import { FormField } from "@/shared/components/form-field/form-field";

/** The inputs of the `number`th Quality, named by its `rowKey`, as `quality` starts them. */
export function QualityFields({
  rowKey: key,
  quality,
  number,
  profiles,
  errors,
  onRemove,
}: {
  rowKey: number;
  quality: Quality;
  number: number;
  profiles: Choice[];
  errors: Record<string, string | undefined>;
  onRemove: () => void;
}) {
  return (
    <fieldset className="flex flex-col gap-4 border-t pt-5">
      <legend className="float-start text-sm font-medium">Quality {number}</legend>
      <ProfileSelect
        name={`profile-${key}`}
        profileId={quality.profileId}
        profiles={profiles}
        error={errors[`profile-${key}`]}
      />
      <FormField
        label="Name"
        name={`name-${key}`}
        defaultValue={quality.name}
        hint="What Requesters see, such as HD or 4K."
        error={errors[`name-${key}`]}
      />
      <FormField
        label="Description"
        name={`description-${key}`}
        defaultValue={quality.description}
        required={false}
        hint="Optional, such as About 50 GB per film."
      />
      <QualityEnd rowKey={key} number={number} isDefault={quality.isDefault} onRemove={onRemove} />
    </fieldset>
  );
}
