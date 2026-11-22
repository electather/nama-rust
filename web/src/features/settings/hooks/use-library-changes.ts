import { useState } from "react";

import { useUpdateLibrary } from "@/features/admin-libraries/data/use-update-library";
import { libraryErrors } from "@/features/admin-libraries/lib/library-refusals";
import { useAutoSave } from "@/features/settings/hooks/use-auto-save";
import type { Library } from "@/gen/nama/v1/library_pb";

/** What a Library's view changes and saves as it's changed. */
export interface LibraryValues {
  name: string;
  folders: string[];
  titleSources: string[];
  recognisers: string[];
  segmentSources: string[];
}

/**
 * `library`'s name, folders and Plugin orders, each change saved as it's made: what's shown, the
 * change to make, what the toast says of a refused one, and whether a change to the folders was
 * saved, which the next scan picks up.
 */
export function useLibraryChanges(library: Library) {
  const update = useUpdateLibrary();
  const [foldersSaved, setFoldersSaved] = useState(false);
  const saved: LibraryValues = {
    name: library.name,
    folders: library.folders,
    titleSources: library.titleSources,
    recognisers: library.recogniserIds,
    segmentSources: library.segmentSourceIds,
  };
  const { values, refused, edit, dismiss } = useAutoSave(saved, async (next, changed) => {
    await update.mutateAsync({
      id: library.id,
      name: next.name,
      folders: next.folders,
      titleSources: next.titleSources,
      // Always set, so the Library keeps exactly the Plugins shown, even none.
      recogniserOrder: { ids: next.recognisers },
      segmentSourceOrder: { ids: next.segmentSources },
    });
    if (changed.includes("folders")) {
      setFoldersSaved(true);
    }
  });
  return { values, edit, says: refused && toastWords(refused.failure), dismiss, foldersSaved };
}

/** What the toast says of the change refused with `failure`, whichever field the refusal names. */
function toastWords(failure: unknown) {
  const { form, name, folders, titleSources, recognisers, segmentSources } = libraryErrors(
    failure,
    "save the Library",
  );
  return [form, name, folders, titleSources, recognisers, segmentSources].find(
    (words) => words !== undefined,
  );
}
