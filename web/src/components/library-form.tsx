import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import type { ReactNode } from "react";

import { ConfirmDialog } from "@/components/confirm-dialog";
import { Choices, FormEnd, FormField, FormTextarea } from "@/components/form-field";
import { type Library, LibraryKind, LibraryService } from "@/gen/nama/v1/library_pb";
import { type FormErrors, formText, onSubmitted } from "@/lib/forms";
import { foldersTyped, librariesKey, libraryErrors } from "@/lib/libraries";

/** What a new Library can hold. */
const kinds: [value: string, label: string][] = [
  ["movies", "Movies"],
  ["shows", "Shows"],
];

/** The form an Admin creates a Library with: its name, kind and folders. */
export function CreateLibrary() {
  const queryClient = useQueryClient();
  const create = useMutation(LibraryService.method.createLibrary, {
    onSuccess: () => queryClient.invalidateQueries({ queryKey: librariesKey }),
  });
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
          },
          {
            onSuccess: () => {
              form.reset();
            },
          },
        );
      })}
    >
      <NameAndFolders errors={errors}>
        <Choices legend="Holds" name="kind" choices={kinds} />
      </NameAndFolders>
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

/** The form an Admin renames `library` or changes its folders with. Its kind can't change. */
export function EditLibrary({ library }: { library: Library }) {
  const queryClient = useQueryClient();
  const update = useMutation(LibraryService.method.updateLibrary, {
    onSuccess: () => queryClient.invalidateQueries({ queryKey: librariesKey }),
  });
  const errors = update.error ? libraryErrors(update.error, "save the Library") : {};

  return (
    <form
      className="mt-4 flex flex-col gap-5"
      onSubmit={onSubmitted((typed) => {
        update.mutate({
          id: library.id,
          name: formText(typed, "name"),
          folders: foldersTyped(formText(typed, "folders")),
        });
      })}
    >
      <NameAndFolders errors={errors} library={library} />
      <FormEnd
        action={update.isPending ? "Saving…" : "Save changes"}
        pending={update.isPending}
        done={update.isSuccess ? "Saved. Scan the Library to pick up new folders." : undefined}
        refusal={errors.form}
      />
    </form>
  );
}

/** The button that deletes `library`, once the Admin confirms, then goes back to the Libraries. */
export function DeleteLibrary({ library }: { library: Library }) {
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const remove = useMutation(LibraryService.method.deleteLibrary, {
    onSuccess: async () => {
      await navigate({ to: "/admin/libraries" });
      await queryClient.invalidateQueries({ queryKey: librariesKey });
    },
  });
  return (
    <ConfirmDialog
      action="Delete"
      name={`Delete ${library.name}`}
      title={`Delete ${library.name}?`}
      description="Its Movies or Shows leave the Server. The files in its folders stay where they are. This can’t be undone."
      onConfirm={() => remove.mutateAsync({ id: library.id })}
      refusal={(error) => libraryErrors(error, "delete the Library").form}
    />
  );
}

/** The name and folders fields, with any `children` between them. */
function NameAndFolders({
  errors,
  library,
  children,
}: {
  errors: FormErrors;
  library?: Library;
  children?: ReactNode;
}) {
  return (
    <>
      <FormField
        label="Name"
        name="name"
        autoComplete="off"
        defaultValue={library?.name}
        error={errors.name}
      />
      {children}
      <FormTextarea
        label="Folders"
        name="folders"
        spellCheck={false}
        defaultValue={library?.folders.join("\n")}
        hint="Where its files are on the Server: one full path per line, such as /srv/media/films."
        error={errors.folders}
      />
    </>
  );
}
