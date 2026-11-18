import { marked, removedFlag } from "@/features/settings/lib/default-quality";
import type { DefaultQuality } from "@/gen/nama/v1/request_pb";
import { FormSelect } from "@/shared/components/form-field/form-select";

/**
 * A dropdown of the Fulfiller's Qualities, starting at the User's default, else the Fulfiller's,
 * which is marked. Flags a default an admin has since removed, as the Fulfiller's applies then.
 */
export function QualitySelect({
  label,
  fulfiller: { qualities, quality },
}: {
  label: string;
  fulfiller: DefaultQuality;
}) {
  const listDefault = qualities.find((offered) => offered.isDefault);
  const yours = qualities.find((offered) => offered.id === quality);
  return (
    <FormSelect
      label={label}
      name="quality"
      defaultValue={String((yours ?? listDefault)?.id)}
      error={removedFlag(quality, yours, listDefault)}
    >
      {qualities.map((offered) => (
        <option key={String(offered.id)} value={String(offered.id)}>
          {marked(offered)}
        </option>
      ))}
    </FormSelect>
  );
}
