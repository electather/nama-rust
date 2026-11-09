import { FormField, FormSelect } from "@/components/form-field";
import { Button } from "@/components/ui/button";
import type { Choice, Quality } from "@/gen/nama/v1/plugin_pb";

/** The button that adds an empty Quality to the list. */
export function AddQuality({ onAdd }: { onAdd: () => void }) {
  return (
    // A ghost button is pulled back by its own padding, so its words line up with the fields.
    <div className="-ms-4">
      <Button type="button" variant="ghost" onClick={onAdd}>
        Add a Quality
      </Button>
    </div>
  );
}

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

/** The end of a Quality's fields: whether it is the default, and removing it. */
function QualityEnd({
  rowKey,
  number,
  isDefault,
  onRemove,
}: {
  rowKey: number;
  number: number;
  isDefault: boolean;
  onRemove: () => void;
}) {
  return (
    <div className="flex items-center justify-between gap-4">
      <label className="flex items-center gap-3 text-sm">
        <input
          type="radio"
          name="default"
          value={rowKey}
          defaultChecked={isDefault}
          className="size-4 accent-foreground"
        />
        Default
      </label>
      {/* Pulled back by its own padding, so its words line up with the fields' end. */}
      <div className="-me-4">
        <Button type="button" variant="ghost" onClick={onRemove}>
          Remove<span className="sr-only"> Quality {number}</span>
        </Button>
      </div>
    </div>
  );
}

/** A Quality's profile, from what the service offers. */
function ProfileSelect({
  name,
  profileId,
  profiles,
  error,
}: {
  name: string;
  profileId: string;
  profiles: Choice[];
  error: string | undefined;
}) {
  // A saved profile the check doesn't offer (now) stays pickable, so saving doesn't drop it.
  const shown =
    profileId === "" || profiles.some(({ value }) => value === profileId)
      ? profiles
      : [...profiles, { value: profileId, label: profileId }];
  return (
    <FormSelect label="Profile" name={name} defaultValue={profileId} error={error}>
      <option value="">Pick one</option>
      {shown.map(({ value, label }) => (
        <option key={value} value={value}>
          {label}
        </option>
      ))}
    </FormSelect>
  );
}
