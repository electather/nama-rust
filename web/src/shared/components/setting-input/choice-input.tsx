import type { Choice, Setting, SettingValue } from "@/gen/nama/v1/plugin_pb";
import { FieldSearchSelect } from "@/shared/components/form-field/field-search-select";
import { FormSelect } from "@/shared/components/form-field/form-select";
import { RowField } from "@/shared/components/form-field/row-field";

/** Past this many choices, a dropdown gets a search: a long list is slow to scroll through. */
const SEARCHED_PAST = 10;

/**
 * A choice setting: a dropdown of its `choices`, those its Manifest lists or the Plugin's last
 * passing test offered, with a search for a long list. Locked while there are none yet, until a
 * test passes, as only the outside service knows what may be picked.
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
  const stored = textOf(setting.value);
  if (choices === undefined) {
    return (
      <FormSelect
        row
        label={setting.label}
        name={setting.key}
        disabled
        hint={[
          setting.description,
          "Test the settings to choose from what the Plugin offers.",
        ].join(" ")}
        error={error}
        options={[{ value: stored, label: stored === "" ? "Not chosen" : stored }]}
        // A disabled dropdown isn't sent, so this sends the stored choice as it was.
        hiddenInput={<input type="hidden" name={setting.key} value={stored} />}
      />
    );
  }
  // Unset, it starts on the default the Plugin runs with, as a Choices setting does.
  const held = textOf(setting.value ?? setting.defaultValue);
  const listed = choices.map(({ value, label }) => ({ value, label }));
  // A held value no longer listed stays an option, so saving another setting never drops it
  // unseen: the Server refuses it instead, saying why.
  const unlisted = held === "" || listed.some((choice) => choice.value === held);
  const options = unlisted ? listed : [...listed, { value: held, label: held }];
  if (options.length > SEARCHED_PAST) {
    return (
      <RowField label={setting.label} hint={setting.description} error={error}>
        {(control) => (
          <FieldSearchSelect
            control={control}
            choice={{ name: setting.key, options, defaultValue: held, required }}
          />
        )}
      </RowField>
    );
  }
  return (
    <FormSelect
      row
      label={setting.label}
      name={setting.key}
      defaultValue={held}
      required={required}
      hint={setting.description}
      error={error}
      options={[{ value: "", label: "Pick one" }, ...options]}
    />
  );
}

/** The text `value` holds, or none. */
function textOf(value: SettingValue | undefined) {
  return value?.value.case === "textValue" ? value.value.value : "";
}
