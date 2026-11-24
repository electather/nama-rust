import { Button } from "nama-ui/components/button";
import { type RefObject, useId } from "react";

import { FindAMatch } from "@/shared/components/find-a-match/find-a-match";
import type { ToMatch } from "@/shared/lib/find-a-match";
import { parsedAs } from "@/shared/lib/unmatched-names";

/** The dialog holding `item`'s Match flow, which searches afresh each time it's `opened`. */
export function MatchDialog({
  dialog,
  opened,
  ...toMatch
}: ToMatch & {
  dialog: RefObject<HTMLDialogElement | null>;
  /** How many times it has opened, so none searches until it first does. */
  opened: number;
}) {
  const heading = useId();
  return (
    <dialog
      ref={dialog}
      aria-labelledby={heading}
      // Its own pointer events: over the player, it inherits none from the title bar while the
      // controls hide.
      className="pointer-events-auto m-auto w-full max-w-sm rounded-xl border bg-popover p-6 text-start text-popover-foreground shadow-xl backdrop:bg-scrim sm:max-w-lg"
    >
      <h2 id={heading} className="mb-4 text-lg font-semibold tracking-tight text-balance">
        Match {parsedAs(toMatch.item)}
      </h2>
      {opened > 0 && <FindAMatch key={opened} {...toMatch} />}
      <div className="mt-6 flex justify-end">
        <Button variant="ghost" onClick={() => dialog.current?.close()}>
          Close
        </Button>
      </div>
    </dialog>
  );
}
