import { useId } from "react";

import { languages } from "@/features/settings/lib/languages";
import { FormField } from "@/shared/components/form-field/form-field";

/**
 * The field labelled `label` a language is typed in, suggesting `none`, for no language, and each
 * language on the list by name.
 */
export function LanguageField({
  label,
  none,
  hint,
  value,
  error,
  onChange,
}: {
  label: string;
  none: string;
  hint: string;
  value: string;
  error: string | undefined;
  onChange: (typed: string) => void;
}) {
  const listId = useId();
  return (
    <>
      <FormField
        label={label}
        name="language"
        required={false}
        list={listId}
        autoComplete="off"
        placeholder={none}
        hint={hint}
        error={error}
        value={value}
        onChange={(event) => {
          onChange(event.target.value);
        }}
      />
      <datalist id={listId}>
        <option value={none}>{none}</option>
        {languages.map(({ code, name }) => (
          <option key={code} value={name}>
            {name}
          </option>
        ))}
      </datalist>
    </>
  );
}
