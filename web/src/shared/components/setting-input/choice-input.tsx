import type { Choice, Setting } from "@/gen/nama/v1/plugin_pb";
import { FormSelect } from "@/shared/components/form-field/form-select";

/**
 * A choice setting: a dropdown of what the Plugin's last passing test offered, locked until a test
 * passes, as only the outside service knows what may be picked.
 */
export function ChoiceInput({
  setting,
  choices,
  error,
  required,
}: {
  setting: Setting;
  choices: Choice[] | undefined;
  error: string | undefined;
  required: boolean;
}) {
  const stored = setting.value?.value.case === "textValue" ? setting.value.value.value : "";
  if (choices === undefined) {
    return (
      <>
        <FormSelect
          label={setting.label}
          name={setting.key}
          disabled
          hint={[
            setting.description,
            "Test the settings to choose from what the Plugin offers.",
          ].join(" ")}
          error={error}
        >
          <option>{stored === "" ? "Not chosen" : stored}</option>
        </FormSelect>
        {/* A disabled dropdown isn't sent, so this sends the stored choice as it was. */}
        <input type="hidden" name={setting.key} value={stored} />
      </>
    );
  }
  return (
    <FormSelect
      label={setting.label}
      name={setting.key}
      defaultValue={stored}
      required={required}
      hint={setting.description}
      error={error}
    >
      <option value="">Pick one</option>
      {choices.map(({ value, label }) => (
        <option key={value} value={value}>
          {label}
        </option>
      ))}
    </FormSelect>
  );
}
