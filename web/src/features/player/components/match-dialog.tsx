import { Button } from "nama-ui/components/button";
import { type RefObject, useId } from "react";

import { FindAMatch } from "@/shared/components/find-a-match/find-a-match";
import { type ItemToMatch, parsedAs } from "@/shared/lib/unmatched-names";

/** The dialog holding `item`'s Match flow, which searches only once `opened`. */
export function MatchDialog({
  dialog,
  item,
  libraryId,
  opened,
}: {
  dialog: RefObject<HTMLDialogElement | null>;
  item: ItemToMatch;
  libraryId: bigint;
  opened: boolean;
}) {
  const heading = useId();
  return (
    <dialog
      ref={dialog}
      aria-labelledby={heading}
      // Its own pointer events: it inherits none from the title bar while the controls hide.
      className="pointer-events-auto m-auto w-full max-w-sm rounded-xl border bg-popover p-6 text-start text-popover-foreground shadow-xl backdrop:bg-scrim sm:max-w-lg"
    >
      <h2 id={heading} className="mb-4 text-lg font-semibold tracking-tight text-balance">
        Match {parsedAs(item)}
      </h2>
      {opened && <FindAMatch item={item} libraryId={libraryId} />}
      <div className="mt-6 flex justify-end">
        <Button variant="ghost" onClick={() => dialog.current?.close()}>
          Close
        </Button>
      </div>
    </dialog>
  );
}
