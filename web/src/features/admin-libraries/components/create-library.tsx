import { useNavigate } from "@tanstack/react-router";
import { useState } from "react";

import { NameAndFolders } from "@/features/admin-libraries/components/name-and-folders";
import { RecognisersField } from "@/features/admin-libraries/components/recognisers-field/recognisers-field";
import { TitleSourcesField } from "@/features/admin-libraries/components/title-sources-field/title-sources-field";
import { useCreateLibrary } from "@/features/admin-libraries/data/use-create-library";
import { foldersTyped } from "@/features/admin-libraries/lib/library-form";
import { libraryErrors } from "@/features/admin-libraries/lib/library-refusals";
import { RECOGNISERS, SEGMENT_SOURCES } from "@/features/admin-libraries/lib/recognisers";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { FormEnd } from "@/shared/components/form-field/form-end";
import { formText, onSubmitted } from "@/shared/lib/forms";

/** The form an Admin creates a Library with, which then opens it in its place. */
export function CreateLibrary() {
  const create = useCreateLibrary();
  const navigate = useNavigate();
  // As the Server would set them.
  const [titleSources, setTitleSources] = useState(["tmdb"]);
  const [recognisers, setRecognisers] = useState<string[]>([]);
  const [segmentSources, setSegmentSources] = useState<string[]>([]);
  const errors = create.error ? libraryErrors(create.error, "create the Library") : {};

  return (
    <form
      className="flex flex-col gap-5"
      onSubmit={onSubmitted((typed) => {
        create.mutate(
          {
            name: formText(typed, "name"),
            kind: formText(typed, "kind") === "shows" ? LibraryKind.SHOWS : LibraryKind.MOVIES,
            folders: foldersTyped(formText(typed, "folders")),
            titleSources,
            recogniserIds: recognisers,
            segmentSourceIds: segmentSources,
          },
          {
            // In place of the form, so Back goes to the list rather than to an emptied form.
            onSuccess: ({ library }) => {
              void navigate({
                to: ".",
                search: (rest) => ({ ...rest, library: String(library?.id) }),
                replace: true,
              });
            },
          },
        );
      })}
    >
      <NameAndFolders errors={errors} />
      <TitleSourcesField
        order={titleSources}
        setOrder={setTitleSources}
        error={errors.titleSources}
      />
      <RecognisersField
        kind={RECOGNISERS}
        order={recognisers}
        setOrder={setRecognisers}
        error={errors.recognisers}
      />
      <RecognisersField
        kind={SEGMENT_SOURCES}
        order={segmentSources}
        setOrder={setSegmentSources}
        error={errors.segmentSources}
      />
      <FormEnd
        action={create.isPending ? "Creating Library…" : "Create Library"}
        pending={create.isPending}
        done={undefined}
        refusal={errors.form}
      />
    </form>
  );
}
