import { NameAndFolders } from "@/features/admin-libraries/components/name-and-folders";
import { RecognisersField } from "@/features/admin-libraries/components/recognisers-field/recognisers-field";
import { useUpdateLibrary } from "@/features/admin-libraries/data/use-update-library";
import {
  foldersTyped,
  recognisersTyped,
  titleSourcesTyped,
} from "@/features/admin-libraries/lib/library-form";
import { libraryErrors } from "@/features/admin-libraries/lib/library-refusals";
import type { Library } from "@/gen/nama/v1/library_pb";
import { FormEnd } from "@/shared/components/form-field/form-end";
import { formText, onSubmitted } from "@/shared/lib/forms";

/** The form an Admin renames `library` or changes its folders with. Its kind can't change. */
export function EditLibrary({ library }: { library: Library }) {
  const update = useUpdateLibrary();
  const errors = update.error ? libraryErrors(update.error, "save the Library") : {};

  return (
    <form
      className="mt-4 flex flex-col gap-5"
      onSubmit={onSubmitted((typed) => {
        update.mutate({
          id: library.id,
          name: formText(typed, "name"),
          folders: foldersTyped(formText(typed, "folders")),
          titleSources: titleSourcesTyped(typed),
          // Always set, so the Library keeps exactly the Recognisers the form shows, even none.
          recogniserOrder: { ids: recognisersTyped(typed) },
        });
      })}
    >
      <NameAndFolders errors={errors} library={library} />
      <RecognisersField initial={library.recogniserIds} error={errors.recognisers} />
      <FormEnd
        action={update.isPending ? "Saving…" : "Save changes"}
        pending={update.isPending}
        done={update.isSuccess ? "Saved. Scan the Library to pick up new folders." : undefined}
        refusal={errors.form}
      />
    </form>
  );
}
