import type { Choice } from "@/gen/nama/v1/plugin_pb";
import { FormSelect } from "@/shared/components/form-field/form-select";

const NONE: Choice[] = [];

/** A Quality's profile, from what the service offers, none while they aren't known. */
export function ProfileSelect({
  profileId,
  profiles = NONE,
}: {
  profileId: string;
  profiles: Choice[] | undefined;
}) {
  // A saved profile the check doesn't offer (now) stays pickable, so saving doesn't drop it.
  const shown =
    profileId === "" || profiles.some(({ value }) => value === profileId)
      ? profiles
      : [...profiles, { value: profileId, label: profileId }];
  return (
    <FormSelect row label="Profile" name="profile" defaultValue={profileId}>
      <option value="">Pick one</option>
      {shown.map(({ value, label }) => (
        <option key={value} value={value}>
          {label}
        </option>
      ))}
    </FormSelect>
  );
}
