import { ProfileSelect } from "@/features/admin-plugins/components/profile-select";
import type { Choice, Quality } from "@/gen/nama/v1/plugin_pb";
import { FormField } from "@/shared/components/form-field/form-field";

/**
 * A Quality's three fields as rows, starting from `quality`: its profile from those the service
 * offers, its name and its description. On the tinted panel that sets them apart from the list
 * they're typed in.
 */
export function QualityFields({
  quality,
  profiles,
}: {
  quality: Pick<Quality, "profileId" | "name" | "description">;
  profiles: Choice[] | undefined;
}) {
  return (
    // The last row gives back its bottom padding, so the panel puts it back inside its tint.
    <div className="flex flex-col rounded-lg bg-muted/50 px-3.5 pb-3.5">
      <ProfileSelect profileId={quality.profileId} profiles={profiles} />
      <FormField
        row
        label="Name"
        name="name"
        defaultValue={quality.name}
        hint="What Requesters see, such as HD or 4K."
      />
      <FormField
        row
        label="Description"
        name="description"
        defaultValue={quality.description}
        required={false}
        hint="Optional, such as About 50 GB per film."
      />
    </div>
  );
}
