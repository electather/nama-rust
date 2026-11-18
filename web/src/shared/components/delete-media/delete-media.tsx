import { useState } from "react";

import { LibraryService } from "@/gen/nama/v1/library_pb";
import { ConfirmDialog } from "@/shared/components/confirm-dialog/confirm-dialog";
import { MayDownloadAgain } from "@/shared/components/delete-media/may-download-again";
import { NotDeleted } from "@/shared/components/delete-media/not-deleted";
import { WhatGoes } from "@/shared/components/delete-media/what-goes";
import { useSensitiveMutation } from "@/shared/hooks/use-sensitive-mutation";
import { type Deletable, EVERY_VERSION, type Outcome, targetOf } from "@/shared/lib/deletable";
import { formText, refusal } from "@/shared/lib/forms";

/**
 * Deletes a title's every Version, or one of them, once confirmed with the files that go, the
 * space they free and the other Episodes those files hold. A file that couldn't be deleted is
 * listed afterwards, with why.
 */
export function DeleteMedia({
  action = "Delete",
  name = action,
  onDeletedEveryVersion,
  ...deletable
}: Deletable & {
  /** What the button says, such as "Delete Show" where "Delete" alone wouldn't say what goes. */
  action?: string;
  /** The button's accessible name, where `action` alone doesn't say what goes, such as "Delete Alien". */
  name?: string;
  /**
   * Called once every Version went, so the page can leave what's gone, with the names of the
   * Fulfillers that couldn't be told, which the page then reports.
   */
  onDeletedEveryVersion: (fulfillersNotTold: string[]) => Promise<unknown>;
}) {
  const [result, setResult] = useState<Outcome>({ notDeleted: [], fulfillersNotTold: [] });
  const remove = useSensitiveMutation(LibraryService.method.deleteMedia);
  return (
    <>
      <ConfirmDialog
        action={action}
        name={name}
        title={`Delete ${deletable.title}?`}
        description="Its files are deleted from the Server for good. This can’t be undone."
        onConfirm={async (typed) => {
          const typedVersion = formText(typed, "version");
          const chosen = typedVersion === "" ? EVERY_VERSION : typedVersion;
          const deleted = await remove.mutateAsync({ target: targetOf(deletable, chosen) });
          if (chosen === EVERY_VERSION && deleted.notDeleted.length === 0) {
            // This may stay mounted for another title, such as the next Season, so the page reports.
            setResult({ notDeleted: [], fulfillersNotTold: [] });
            await onDeletedEveryVersion(deleted.fulfillersNotTold);
            return;
          }
          setResult(deleted);
        }}
        refusal={(error) => refusal(error, "delete it", {}).form}
      >
        <WhatGoes {...deletable} />
      </ConfirmDialog>
      <NotDeleted files={result.notDeleted} />
      <MayDownloadAgain title={deletable.title} fulfillers={result.fulfillersNotTold} />
    </>
  );
}
