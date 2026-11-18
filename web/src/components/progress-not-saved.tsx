import { useRouteContext } from "@tanstack/react-router";
import { useId, useRef, useState } from "react";

import { FindAMatch, type ItemToMatch, parsedAs } from "@/components/unmatched-item";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { may } from "@/shared/lib/permissions";
import { Button } from "@/shared/ui/button";

/**
 * Under the player's title bar: that `item`, Unmatched, keeps no Watch State (ADR 0013). With
 * Manage Libraries, Match it opens its Match flow.
 */
export function ProgressNotSaved({ item, libraryId }: { item: ItemToMatch; libraryId: bigint }) {
  const { signedIn } = useRouteContext({ from: "/_member" });
  return (
    // Clickable only while the controls show, as the title bar is.
    <div className="pointer-events-auto mt-2 flex justify-center px-4 group-data-controls-hidden:pointer-events-none">
      <div className="flex max-w-md flex-wrap items-center justify-center gap-x-2 rounded-lg bg-scrim px-4 py-1 text-center text-sm text-balance">
        <p className="py-1">Not matched, so progress isn’t saved.</p>
        {may(signedIn.user, Permission.MANAGE_LIBRARIES) && (
          <MatchIt item={item} libraryId={libraryId} />
        )}
      </div>
    </div>
  );
}

/** "Match it", which opens `item`'s Match flow in a dialog over the player. */
function MatchIt({ item, libraryId }: { item: ItemToMatch; libraryId: bigint }) {
  const dialog = useRef<HTMLDialogElement>(null);
  const heading = useId();
  // Searches only once opened, so playing alone asks the title source nothing.
  const [opened, setOpened] = useState(false);
  return (
    <>
      <Button
        variant="overlay"
        aria-haspopup="dialog"
        onClick={() => {
          setOpened(true);
          dialog.current?.showModal();
        }}
      >
        Match it
      </Button>
      {/* Modal, so the player's shortcuts leave the keys typed in it alone (`useShortcuts`). */}
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
    </>
  );
}
