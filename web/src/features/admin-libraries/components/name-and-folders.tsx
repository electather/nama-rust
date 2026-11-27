import { useState } from "react";

import {
  foldersHint,
  foldersTyped,
  foldersWith,
} from "@/features/admin-libraries/lib/library-form";
import type { LibraryField } from "@/features/admin-libraries/lib/library-refusals";
import { FolderPicker } from "@/shared/components/folder-picker";
import { Choices } from "@/shared/components/form-field/choices";
import { FormField } from "@/shared/components/form-field/form-field";
import { FormTextarea } from "@/shared/components/form-field/form-textarea";
import type { FormErrors } from "@/shared/lib/forms";

const kinds: [value: string, label: string][] = [
  ["movies", "Movies"],
  ["shows", "Shows"],
];

/**
 * A new Library's name, what it holds, which can't change after, and its folders, where a folder
 * picked beside them is added as a new line unless it's listed already.
 */
export function NameAndFolders({ errors }: { errors: FormErrors<LibraryField> }) {
  const [folders, setFolders] = useState("");
  const typed = foldersTyped(folders);
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
        value={folders}
        onChange={(event) => {
          setFolders(event.currentTarget.value);
        }}
        aside={
          <FolderPicker
            path={typed.at(-1) ?? ""}
            onPick={(picked) => {
              setFolders(foldersWith(typed, picked).join("\n"));
            }}
          />
        }
      />
    </>
  );
}
