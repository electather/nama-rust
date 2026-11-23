import { Button } from "nama-ui/components/button";
import { useRef, useState } from "react";

import { MatchDialog } from "@/shared/components/find-a-match/match-dialog";
import type { ItemToMatch } from "@/shared/lib/unmatched-names";

/** "Match it", which opens `item`'s Match flow in a dialog over the player. */
export function MatchIt({ item, libraryId }: { item: ItemToMatch; libraryId: bigint }) {
  const dialog = useRef<HTMLDialogElement>(null);
  // Searches only once opened, so playing alone asks the title source nothing.
  const [opened, setOpened] = useState(0);
  return (
    <>
      <Button
        variant="overlay"
        aria-haspopup="dialog"
        onClick={() => {
          setOpened(opened + 1);
          dialog.current?.showModal();
        }}
      >
        Match it
      </Button>
      {/* Modal, so the player's shortcuts leave the keys typed in it alone (`useShortcuts`). */}
      <MatchDialog dialog={dialog} item={item} libraryId={libraryId} opened={opened} />
    </>
  );
}
