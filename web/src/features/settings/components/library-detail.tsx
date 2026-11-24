import { ghostHang } from "nama-ui/components/button";
import { useId } from "react";

import { DeleteLibrary } from "@/features/admin-libraries/components/delete-library";
import { useLibrary } from "@/features/admin-libraries/data/use-library";
import { LibraryBackdrop } from "@/features/settings/components/library-backdrop";
import { LibraryFields } from "@/features/settings/components/library-fields";
import { LibraryHeader } from "@/features/settings/components/library-header";
import { LibraryScan } from "@/features/settings/components/library-scan";
import { SaveToast } from "@/features/settings/components/save-toast";
import { useLibraryChanges } from "@/features/settings/hooks/use-library-changes";
import type { Library } from "@/gen/nama/v1/library_pb";

/**
 * The Library `listed` names, in place of the list: its folders and scan, its Backdrop, then what
 * it's called, where its files are and which Plugins it asks, saved as they're changed, and Delete.
 */
export function LibraryDetail({ listed }: { listed: Library }) {
  const titleId = useId();
  // Its own, so a finished scan refetches its empty folders and last scan.
  const library = useLibrary(listed.id) ?? listed;
  const { values, edit, says, dismiss, foldersSaved } = useLibraryChanges(library);
  const body = (
    <div className="flex flex-col gap-6 px-4 pt-6 pb-4 md:px-6 md:pb-6">
      <LibraryScan library={library} foldersSaved={foldersSaved} />
      <LibraryBackdrop library={library} />
      <LibraryFields values={values} onEdit={edit} />
      <div className={ghostHang}>
        <DeleteLibrary library={library} />
      </div>
    </div>
  );
  return (
    <>
      <section aria-labelledby={titleId} className="flex flex-col">
        <LibraryHeader titleId={titleId} library={library} />
        {body}
      </section>
      <SaveToast says={says} onDismiss={dismiss} />
    </>
  );
}
