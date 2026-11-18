import type { Choice } from "@/gen/nama/v1/plugin_pb";
import { FormSelect } from "@/shared/components/form-field/form-select";

/** A Quality's profile, from what the service offers. */
export function ProfileSelect({
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
