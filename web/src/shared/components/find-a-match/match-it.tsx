import { Button } from "nama-ui/components/button";
import { useRef, useState } from "react";

import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { MatchDialog } from "@/shared/components/find-a-match/match-dialog";
import { useMay } from "@/shared/hooks/use-may";
import type { ItemToMatch } from "@/shared/lib/unmatched-names";

/**
 * A button, "Match it" unless `label` says otherwise, which opens `item`'s Match flow in a dialog.
 * Nothing for a User without Manage Libraries.
 */
export function MatchIt({
  item,
  libraryId,
  variant = "overlay",
  label = "Match it",
}: {
  item: ItemToMatch;
  libraryId: bigint;
  variant?: "overlay" | "ghost";
  label?: string;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  // Searches only once opened, so playing alone asks the title source nothing.
  const [opened, setOpened] = useState(0);
  const mayMatch = useMay(Permission.MANAGE_LIBRARIES);
  if (!mayMatch) {
    return null;
  }
  return (
    <>
      <Button
        variant={variant}
        aria-haspopup="dialog"
        onClick={() => {
          setOpened(opened + 1);
          dialog.current?.showModal();
        }}
      >
        {label}
      </Button>
      {/* Modal, so the player's shortcuts leave the keys typed in it alone (`useShortcuts`). */}
      <MatchDialog dialog={dialog} item={item} libraryId={libraryId} opened={opened} />
    </>
  );
}
