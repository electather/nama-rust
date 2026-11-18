import { NameAndFolders } from "@/features/admin-libraries/components/name-and-folders";
import { RecognisersField } from "@/features/admin-libraries/components/recognisers-field/recognisers-field";
import { useCreateLibrary } from "@/features/admin-libraries/data/use-create-library";
import {
  foldersTyped,
  recognisersTyped,
  titleSourcesTyped,
} from "@/features/admin-libraries/lib/library-form";
import { libraryErrors } from "@/features/admin-libraries/lib/library-refusals";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { FormEnd } from "@/shared/components/form-field/form-end";
import { formText, onSubmitted } from "@/shared/lib/forms";

/** The form an Admin creates a Library with. */
export function CreateLibrary() {
  const create = useCreateLibrary();
  const errors = create.error ? libraryErrors(create.error, "create the Library") : {};

  return (
    <form
      className="mt-4 flex flex-col gap-5"
      onSubmit={onSubmitted((typed, form) => {
        create.mutate(
          {
            name: formText(typed, "name"),
            kind: formText(typed, "kind") === "shows" ? LibraryKind.SHOWS : LibraryKind.MOVIES,
            folders: foldersTyped(formText(typed, "folders")),
            titleSources: titleSourcesTyped(typed),
            recogniserIds: recognisersTyped(typed),
          },
          {
            onSuccess: () => {
              form.reset();
            },
          },
        );
      })}
    >
      <NameAndFolders errors={errors} />
      <RecognisersField error={errors.recognisers} />
      <FormEnd
        action={create.isPending ? "Creating Library…" : "Create Library"}
        pending={create.isPending}
        done={
          create.isSuccess ? `${create.variables.name} is ready. Scan it to fill it.` : undefined
        }
        refusal={errors.form}
      />
    </form>
  );
}
