import { create } from "@bufbuild/protobuf";

import { SwitchRow } from "@/features/settings/components/switch-row";
import { type MaximumAge, MaximumAgeSchema } from "@/gen/nama/v1/sign_in_pb";
import { FormSelect } from "@/shared/components/form-field/form-select";

const NONE = "none";

// Every age up to 18, the oldest any ratings board's Content Rating calls for
// (`crates/nama-server/src/content_ratings.rs`), so an Admin can pick a child's own age.
const options = [
  { value: NONE, label: "None" },
  ...Array.from({ length: 19 }, (_, age) => ({ value: String(age), label: String(age) })),
];

/**
 * A User's Maximum Age, unset for none, as a row and whether unrated titles reach them as another,
 * each pick going to `onChange`, to save at once.
 */
export function MaximumAgeField({
  maximumAge,
  onChange,
}: {
  maximumAge: MaximumAge | undefined;
  onChange: (changes: { maximumAge: MaximumAge | undefined }) => void;
}) {
  const age = maximumAge?.age;
  const allowUnrated = maximumAge?.allowUnrated === true;
  const change = (picked: number | undefined, allow: boolean) => {
    onChange({
      maximumAge:
        picked === undefined
          ? undefined
          : create(MaximumAgeSchema, { age: picked, allowUnrated: allow }),
    });
  };
  return (
    <>
      <FormSelect
        row
        label="Maximum Age"
        hint="The highest age a title’s Content Rating may call for and still reach them."
        name="maximumAge"
        value={age === undefined ? NONE : String(age)}
        onValueChange={(value) => {
          change(value === NONE ? undefined : Number(value), allowUnrated);
        }}
        options={options}
      />
      <SwitchRow
        label="Allow unrated titles"
        hint="Whether a title with no Content Rating reaches them under a Maximum Age."
        checked={allowUnrated}
        disabled={age === undefined}
        onChange={(checked) => {
          change(age, checked);
        }}
      />
    </>
  );
}
