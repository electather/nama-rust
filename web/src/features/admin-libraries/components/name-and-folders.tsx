import { foldersHint } from "@/features/admin-libraries/lib/library-form";
import { Choices } from "@/shared/components/form-field/choices";
import { FormField } from "@/shared/components/form-field/form-field";
import { FormTextarea } from "@/shared/components/form-field/form-textarea";
import type { FormErrors } from "@/shared/lib/forms";

const kinds: [value: string, label: string][] = [
  ["movies", "Movies"],
  ["shows", "Shows"],
];

/** A new Library's name, what it holds, which can't change after, and its folders. */
export function NameAndFolders({ errors }: { errors: FormErrors }) {
  return (
    <>
      <FormField label="Name" name="name" autoComplete="off" error={errors.name} />
      <Choices legend="Holds" name="kind" choices={kinds} />
      <FormTextarea
        label="Folders"
        name="folders"
        spellCheck={false}
        hint={foldersHint}
        error={errors.folders}
      />
    </>
  );
}
