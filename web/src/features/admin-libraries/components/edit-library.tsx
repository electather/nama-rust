import { NameAndFolders } from "@/features/admin-libraries/components/name-and-folders";
import { RecognisersField } from "@/features/admin-libraries/components/recognisers-field/recognisers-field";
import { useUpdateLibrary } from "@/features/admin-libraries/data/use-update-library";
import {
  foldersTyped,
  pluginOrderTyped,
  titleSourcesTyped,
} from "@/features/admin-libraries/lib/library-form";
import { libraryErrors } from "@/features/admin-libraries/lib/library-refusals";
import { RECOGNISERS, SEGMENT_SOURCES } from "@/features/admin-libraries/lib/recognisers";
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
          // Always set, so the Library keeps exactly the Plugins the form shows, even none.
          recogniserOrder: { ids: pluginOrderTyped(typed, RECOGNISERS) },
          segmentSourceOrder: { ids: pluginOrderTyped(typed, SEGMENT_SOURCES) },
        });
      })}
    >
      <NameAndFolders errors={errors} library={library} />
      <RecognisersField
        kind={RECOGNISERS}
        initial={library.recogniserIds}
        error={errors.recognisers}
      />
      <RecognisersField
        kind={SEGMENT_SOURCES}
        initial={library.segmentSourceIds}
        error={errors.segmentSources}
      />
      <FormEnd
        action={update.isPending ? "Saving…" : "Save changes"}
        pending={update.isPending}
        done={update.isSuccess ? "Saved. Scan the Library to pick up new folders." : undefined}
        refusal={errors.form}
      />
    </form>
  );
}
